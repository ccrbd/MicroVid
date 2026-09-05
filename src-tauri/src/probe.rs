//! ffprobe wrapper → MediaInfo.
use crate::ffmpeg::hide_console_tokio;
use crate::models::{AudioStream, MediaInfo, SubtitleStream, VideoStream};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::Path;

pub const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "wmv", "ts", "m2ts", "mts", "webm", "flv", "mpg", "mpeg", "vob", "ogv", "3gp", "divx",
];

pub fn is_video_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

const TEXT_SUB_CODECS: &[&str] = &["subrip", "srt", "ass", "ssa", "webvtt", "mov_text", "text", "ttml"];

fn parse_ratio(s: &str) -> Option<f64> {
    let (a, b) = s.split_once([':', '/'])?;
    let a: f64 = a.trim().parse().ok()?;
    let b: f64 = b.trim().parse().ok()?;
    if b == 0.0 {
        None
    } else {
        Some(a / b)
    }
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
}
fn u64_of(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
}
fn f64_of(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())))
}
fn tag(v: &Value, k: &str) -> Option<String> {
    v.get("tags").and_then(|t| t.get(k)).and_then(|x| x.as_str()).map(|s| s.to_string())
}
fn disposition(v: &Value, k: &str) -> bool {
    v.get("disposition").and_then(|d| d.get(k)).and_then(|x| x.as_i64()).unwrap_or(0) == 1
}

fn bit_depth_of(pix_fmt: &str, bits_per_raw: Option<u64>) -> u8 {
    if let Some(b) = bits_per_raw {
        if b > 0 {
            return b as u8;
        }
    }
    if pix_fmt.contains("10") {
        10
    } else if pix_fmt.contains("12") {
        12
    } else {
        8
    }
}

pub fn parse_ffprobe_json(path: &Path, json: &Value) -> Result<MediaInfo> {
    let format = json.get("format").cloned().unwrap_or(Value::Null);
    let streams = json.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let duration_secs = f64_of(&format, "duration").unwrap_or(0.0);
    let mut video = None;
    let mut audio = vec![];
    let mut subtitles = vec![];
    let (mut a_rel, mut s_rel) = (0usize, 0usize);
    for s in &streams {
        let index = s.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let codec = str_of(s, "codec_name").unwrap_or_default();
        match s.get("codec_type").and_then(|x| x.as_str()).unwrap_or("") {
            "video" => {
                // Skip attached pictures (cover art).
                if disposition(s, "attached_pic") || video.is_some() {
                    continue;
                }
                let fps = str_of(s, "avg_frame_rate")
                    .and_then(|r| parse_ratio(&r))
                    .filter(|f| *f > 0.0 && f.is_finite())
                    .or_else(|| str_of(s, "r_frame_rate").and_then(|r| parse_ratio(&r)))
                    .unwrap_or(24.0);
                let sar = str_of(s, "sample_aspect_ratio").and_then(|r| parse_ratio(&r)).filter(|v| *v > 0.0).unwrap_or(1.0);
                let pix_fmt = str_of(s, "pix_fmt").unwrap_or_default();
                let transfer = str_of(s, "color_transfer").unwrap_or_default();
                video = Some(VideoStream {
                    index,
                    codec,
                    profile: str_of(s, "profile"),
                    width: u64_of(s, "width").unwrap_or(0) as u32,
                    height: u64_of(s, "height").unwrap_or(0) as u32,
                    fps,
                    sar,
                    bit_depth: bit_depth_of(&pix_fmt, u64_of(s, "bits_per_raw_sample")),
                    pix_fmt,
                    bitrate: u64_of(s, "bit_rate"),
                    hdr: transfer == "smpte2084" || transfer == "arib-std-b67",
                });
            }
            "audio" => {
                audio.push(AudioStream {
                    index,
                    rel_index: a_rel,
                    codec,
                    channels: u64_of(s, "channels").unwrap_or(2) as u32,
                    channel_layout: str_of(s, "channel_layout"),
                    language: tag(s, "language"),
                    title: tag(s, "title"),
                    bitrate: u64_of(s, "bit_rate"),
                    is_default: disposition(s, "default"),
                });
                a_rel += 1;
            }
            "subtitle" => {
                let text_based = TEXT_SUB_CODECS.contains(&codec.as_str());
                subtitles.push(SubtitleStream {
                    index,
                    rel_index: s_rel,
                    codec,
                    language: tag(s, "language"),
                    title: tag(s, "title"),
                    forced: disposition(s, "forced"),
                    is_default: disposition(s, "default"),
                    text_based,
                });
                s_rel += 1;
            }
            _ => {}
        }
    }
    let size_bytes = u64_of(&format, "size").or_else(|| std::fs::metadata(path).ok().map(|m| m.len())).unwrap_or(0);
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        size_bytes,
        duration_secs,
        container: str_of(&format, "format_name").unwrap_or_default(),
        bitrate: u64_of(&format, "bit_rate"),
        video,
        audio,
        subtitles,
        chapters: json.get("chapters").and_then(|c| c.as_array()).map(|c| c.len()).unwrap_or(0),
    })
}

pub async fn probe(ffprobe: &Path, path: &Path) -> Result<MediaInfo> {
    let mut cmd = tokio::process::Command::new(ffprobe);
    cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters"]).arg(path);
    hide_console_tokio(&mut cmd);
    let out = cmd.output().await.context("running ffprobe")?;
    if !out.status.success() {
        return Err(anyhow!("ffprobe failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let json: Value = serde_json::from_slice(&out.stdout).context("parsing ffprobe json")?;
    let info = parse_ffprobe_json(path, &json)?;
    if info.video.is_none() {
        return Err(anyhow!("no video stream found"));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_probe() {
        let json: Value = serde_json::from_str(r#"{
          "streams": [
            {"index":0,"codec_type":"video","codec_name":"h264","profile":"High","width":1920,"height":1080,
             "pix_fmt":"yuv420p","avg_frame_rate":"24000/1001","sample_aspect_ratio":"1:1","disposition":{"default":1,"attached_pic":0}},
            {"index":1,"codec_type":"audio","codec_name":"ac3","channels":6,"channel_layout":"5.1","bit_rate":"384000",
             "tags":{"language":"eng"},"disposition":{"default":1}},
            {"index":2,"codec_type":"subtitle","codec_name":"subrip","tags":{"language":"eng","title":"English"},"disposition":{"default":0,"forced":0}},
            {"index":3,"codec_type":"subtitle","codec_name":"hdmv_pgs_subtitle","tags":{"language":"fre"},"disposition":{"default":0,"forced":1}}
          ],
          "format": {"format_name":"matroska,webm","duration":"3492.5","size":"1500000000","bit_rate":"3435000"},
          "chapters": [{"id":0},{"id":1}]
        }"#).unwrap();
        let info = parse_ffprobe_json(Path::new("/tmp/x.mkv"), &json).unwrap();
        let v = info.video.unwrap();
        assert_eq!((v.width, v.height), (1920, 1080));
        assert!((v.fps - 23.976).abs() < 0.001);
        assert_eq!(v.bit_depth, 8);
        assert_eq!(info.audio[0].channels, 6);
        assert_eq!(info.subtitles.len(), 2);
        assert!(info.subtitles[0].text_based);
        assert!(!info.subtitles[1].text_based);
        assert!(info.subtitles[1].forced);
        assert_eq!(info.chapters, 2);
        assert_eq!(info.size_bytes, 1_500_000_000);
    }

    #[test]
    fn detects_anamorphic_and_10bit() {
        let json: Value = serde_json::from_str(r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc","width":1440,"height":1080,
          "pix_fmt":"yuv420p10le","avg_frame_rate":"25/1","sample_aspect_ratio":"4:3"}],"format":{"duration":"10"}}"#).unwrap();
        let info = parse_ffprobe_json(Path::new("/tmp/y.mkv"), &json).unwrap();
        let v = info.video.unwrap();
        assert!((v.sar - 4.0 / 3.0).abs() < 1e-9);
        assert!((v.dar_of(v.width, v.height) - 16.0 / 9.0).abs() < 1e-9);
        assert_eq!(v.bit_depth, 10);
    }
}
