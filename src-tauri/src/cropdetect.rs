//! Black-bar detection: sample three points, keep the most conservative crop.
use crate::ffmpeg::hide_console_tokio;
use crate::models::{Crop, MediaInfo};
use regex::Regex;
use std::path::Path;

pub fn parse_crop_lines(stderr: &str) -> Option<Crop> {
    let re = Regex::new(r"crop=(\d+):(\d+):(\d+):(\d+)").ok()?;
    // Take the last reported crop of this sample (cropdetect converges over frames).
    let caps = re.captures_iter(stderr).last()?;
    Some(Crop {
        w: caps[1].parse().ok()?,
        h: caps[2].parse().ok()?,
        x: caps[3].parse().ok()?,
        y: caps[4].parse().ok()?,
    })
}

/// Choose the safest crop across samples: the one covering the largest area, then
/// reject it when it barely changes anything or looks like a false positive.
pub fn choose(samples: &[Crop], width: u32, height: u32) -> Option<Crop> {
    let best = samples.iter().copied().max_by_key(|c| (c.w as u64) * (c.h as u64))?;
    if best.w == 0 || best.h == 0 {
        return None;
    }
    let removed_h = height.saturating_sub(best.h);
    let removed_w = width.saturating_sub(best.w);
    if removed_h < 16 && removed_w < 16 {
        return None;
    }
    // Never crop more than 40% of either dimension (dark scenes fool cropdetect).
    if (best.h as f64) < (height as f64) * 0.6 || (best.w as f64) < (width as f64) * 0.6 {
        return None;
    }
    Some(Crop { w: best.w & !1, h: best.h & !1, x: best.x & !1, y: best.y & !1 })
}

pub async fn detect(ffmpeg: &Path, info: &MediaInfo) -> Option<Crop> {
    let v = info.video.as_ref()?;
    let dur = info.duration_secs;
    if dur < 10.0 {
        return None;
    }
    let mut samples = vec![];
    for frac in [0.2, 0.5, 0.8] {
        let ss = dur * frac;
        let mut cmd = tokio::process::Command::new(ffmpeg);
        cmd.args(["-hide_banner", "-nostdin", "-loglevel", "info", "-ss", &format!("{ss:.2}"), "-t", "3", "-i", &info.path, "-an", "-sn", "-vf", "cropdetect=24:2:0", "-f", "null", "-"]);
        hide_console_tokio(&mut cmd);
        if let Ok(out) = cmd.output().await {
            if let Some(c) = parse_crop_lines(&String::from_utf8_lossy(&out.stderr)) {
                samples.push(c);
            }
        }
    }
    choose(&samples, v.width, v.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_last_crop_line() {
        let s = "[Parsed_cropdetect_0 @ 0x1] x1:0 x2:1919 y1:140 y2:939 w:1920 h:800 x:0 y:140 pts:1 t:0.04 crop=1920:800:0:140\nfoo crop=1920:796:0:142\n";
        assert_eq!(parse_crop_lines(s), Some(Crop { w: 1920, h: 796, x: 0, y: 142 }));
    }

    #[test]
    fn picks_safest_and_rejects_tiny_or_huge() {
        let a = Crop { w: 1920, h: 800, x: 0, y: 140 };
        let b = Crop { w: 1920, h: 816, x: 0, y: 132 };
        assert_eq!(choose(&[a, b], 1920, 1080), Some(b));
        assert_eq!(choose(&[Crop { w: 1920, h: 1072, x: 0, y: 4 }], 1920, 1080), None);
        assert_eq!(choose(&[Crop { w: 1920, h: 400, x: 0, y: 340 }], 1920, 1080), None);
    }
}
