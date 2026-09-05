//! Locating the ffmpeg/ffprobe binaries, probing their capabilities and parsing
//! the `-progress pipe:1` key/value stream.
use crate::models::Capabilities;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Candidate locations in priority order: custom path → bundled sidecar → PATH.
fn candidates(tool: &str, custom_dir: Option<&Path>) -> Vec<(PathBuf, &'static str)> {
    let mut out = vec![];
    if let Some(dir) = custom_dir {
        out.push((dir.join(exe(tool)), "custom"));
    }
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            out.push((dir.join(exe(tool)), "bundled"));
            // macOS .app: Contents/MacOS/<exe>, sidecars live next to it.
            out.push((dir.join("..").join("Resources").join(exe(tool)), "bundled"));
        }
    }
    // Dev fallback: the sidecar in the source tree.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let triple = current_triple();
        out.push((PathBuf::from(manifest).join("binaries").join(exe(&format!("{tool}-{triple}"))), "bundled"));
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries").join(exe(&format!("{tool}-{}", current_triple())));
    out.push((dev, "bundled"));
    out.push((PathBuf::from(exe(tool)), "path"));
    out
}

pub fn current_triple() -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => other,
    };
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        other => format!("{arch}-{other}"),
    }
}

fn works(path: &Path) -> bool {
    let mut cmd = Command::new(path);
    cmd.arg("-version");
    hide_console(&mut cmd);
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

#[cfg(windows)]
pub fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
pub fn hide_console(_cmd: &mut Command) {}

#[cfg(windows)]
pub fn hide_console_tokio(cmd: &mut tokio::process::Command) {
    cmd.creation_flags(0x0800_0000);
}
#[cfg(not(windows))]
pub fn hide_console_tokio(_cmd: &mut tokio::process::Command) {}

/// Resolve both binaries. `custom` may be a directory or a path to the ffmpeg binary.
pub fn resolve(custom: Option<&str>) -> Result<(PathBuf, PathBuf, String)> {
    let custom_dir = custom.filter(|s| !s.trim().is_empty()).map(|s| {
        let p = PathBuf::from(s.trim());
        if p.is_file() {
            p.parent().map(|d| d.to_path_buf()).unwrap_or(p)
        } else {
            p
        }
    });
    let mut ffmpeg = None;
    for (p, src) in candidates("ffmpeg", custom_dir.as_deref()) {
        if works(&p) {
            ffmpeg = Some((p, src));
            break;
        }
    }
    let (ffmpeg, source) = ffmpeg.ok_or_else(|| anyhow!("ffmpeg not found (bundled sidecar missing and none on PATH)"))?;
    let mut ffprobe = None;
    // Prefer the ffprobe sitting next to the chosen ffmpeg.
    if let Some(dir) = ffmpeg.parent() {
        let p = dir.join(exe("ffprobe"));
        if works(&p) {
            ffprobe = Some(p);
        }
        // Dev sidecar naming: ffprobe-<triple>
        let p = dir.join(exe(&format!("ffprobe-{}", current_triple())));
        if ffprobe.is_none() && works(&p) {
            ffprobe = Some(p);
        }
    }
    if ffprobe.is_none() {
        for (p, _) in candidates("ffprobe", custom_dir.as_deref()) {
            if works(&p) {
                ffprobe = Some(p);
                break;
            }
        }
    }
    let ffprobe = ffprobe.ok_or_else(|| anyhow!("ffprobe not found next to ffmpeg or on PATH"))?;
    Ok((ffmpeg, ffprobe, source.to_string()))
}

pub fn capabilities(custom: Option<&str>) -> Result<Capabilities> {
    let (ffmpeg, ffprobe, source) = resolve(custom)?;
    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-hide_banner", "-version"]);
    hide_console(&mut cmd);
    let ver_out = cmd.output().context("running ffmpeg -version")?;
    let version = String::from_utf8_lossy(&ver_out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches("ffmpeg version ")
        .split_whitespace()
        .next()
        .unwrap_or("?")
        .to_string();

    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-hide_banner", "-encoders"]);
    hide_console(&mut cmd);
    let enc_out = cmd.output().context("running ffmpeg -encoders")?;
    let text = String::from_utf8_lossy(&enc_out.stdout);
    let encoders: Vec<String> = text
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            // lines look like " V....D libx264              H.264 ..."
            let mut parts = l.split_whitespace();
            let flags = parts.next()?;
            let name = parts.next()?;
            if flags.len() == 6 && flags.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();
    let has = |n: &str| encoders.iter().any(|e| e == n);
    let pick = |names: &[&str]| names.iter().find(|n| has(n)).map(|s| s.to_string());
    Ok(Capabilities {
        ffmpeg_path: ffmpeg.to_string_lossy().into_owned(),
        ffprobe_path: ffprobe.to_string_lossy().into_owned(),
        source,
        version,
        has_aac_at: has("aac_at"),
        has_fdk_aac: has("libfdk_aac"),
        has_x264: has("libx264"),
        has_x265: has("libx265"),
        has_svtav1: has("libsvtav1"),
        hw_h264: pick(&["h264_videotoolbox", "h264_nvenc", "h264_qsv", "h264_amf"]),
        hw_hevc: pick(&["hevc_videotoolbox", "hevc_nvenc", "hevc_qsv", "hevc_amf"]),
        encoders,
    })
}

/// One parsed block of `-progress` output.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProgressLine {
    pub frame: Option<u64>,
    pub fps: Option<f64>,
    pub total_size: Option<u64>,
    pub out_time_secs: Option<f64>,
    pub speed: Option<f64>,
    pub end: bool,
}

/// Feed a single `key=value` line. Returns Some when the block is complete (`progress=...`).
pub fn parse_progress_line(acc: &mut ProgressLine, line: &str) -> Option<ProgressLine> {
    let line = line.trim();
    let Some((k, v)) = line.split_once('=') else { return None };
    let v = v.trim();
    match k.trim() {
        "frame" => acc.frame = v.parse().ok(),
        "fps" => acc.fps = v.parse().ok(),
        "total_size" => acc.total_size = v.parse().ok(),
        "out_time_us" => acc.out_time_secs = v.parse::<f64>().ok().map(|us| us / 1_000_000.0),
        "out_time_ms" if acc.out_time_secs.is_none() => {
            // Older ffmpeg reports microseconds under this key despite the name.
            acc.out_time_secs = v.parse::<f64>().ok().map(|us| us / 1_000_000.0)
        }
        "speed" => acc.speed = v.trim_end_matches('x').trim().parse().ok(),
        "progress" => {
            acc.end = v == "end";
            let done = std::mem::take(acc);
            return Some(done);
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_block() {
        let mut acc = ProgressLine::default();
        let lines = ["frame=120", "fps=48.5", "total_size=123456", "out_time_us=5000000", "speed=2.02x", "progress=continue"];
        let mut result = None;
        for l in lines {
            if let Some(r) = parse_progress_line(&mut acc, l) {
                result = Some(r);
            }
        }
        let r = result.expect("block");
        assert_eq!(r.frame, Some(120));
        assert_eq!(r.fps, Some(48.5));
        assert_eq!(r.total_size, Some(123456));
        assert_eq!(r.out_time_secs, Some(5.0));
        assert_eq!(r.speed, Some(2.02));
        assert!(!r.end);
    }

    #[test]
    fn handles_na_values_and_end() {
        let mut acc = ProgressLine::default();
        parse_progress_line(&mut acc, "speed=N/A");
        parse_progress_line(&mut acc, "fps=0.0");
        let r = parse_progress_line(&mut acc, "progress=end").unwrap();
        assert_eq!(r.speed, None);
        assert!(r.end);
    }
}
