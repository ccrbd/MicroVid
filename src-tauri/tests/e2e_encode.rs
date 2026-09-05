//! End-to-end: real ffmpeg, real files. Run with `cargo test --test e2e_encode -- --ignored`
//! and MICROVID_SAMPLE=/path/to/Show\ S01E01.mkv (a file with black bars, 5.1 audio and an embedded sub).
use std::path::{Path, PathBuf};
use std::process::Command;

fn ffprobe_streams(ffprobe: &str, path: &Path) -> serde_json::Value {
    let out = Command::new(ffprobe).args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"]).arg(path).output().unwrap();
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
#[ignore]
fn encodes_sample_with_subs_crop_and_downmix() {
    let sample = PathBuf::from(std::env::var("MICROVID_SAMPLE").expect("MICROVID_SAMPLE"));
    let caps = microvid_lib::ffmpeg::capabilities(None).expect("ffmpeg");
    assert!(caps.has_x265 && caps.has_x264, "need x264/x265 in {}", caps.ffmpeg_path);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let info = rt.block_on(microvid_lib::probe::probe(Path::new(&caps.ffprobe_path), &sample)).expect("probe");
    let v = info.video.as_ref().unwrap();
    assert_eq!((v.width, v.height), (1920, 1080));
    assert_eq!(info.audio[0].channels, 6);
    assert_eq!(info.subtitles.len(), 1);

    let crop = rt.block_on(microvid_lib::cropdetect::detect(Path::new(&caps.ffmpeg_path), &info));
    let crop = crop.expect("black bars should be detected");
    assert!(crop.h <= 800 + 8 && crop.h >= 800 - 8, "crop {crop:?}");

    let cands = microvid_lib::subtitles::find_candidates(&sample);
    assert!(!cands.is_empty(), "sidecar .en.srt should be found");
    let sub = PathBuf::from(&cands[0].path);

    let settings = microvid_lib::models::EncodeSettings { preset: Some("ultrafast".into()), ..Default::default() };
    let out_dir = std::env::temp_dir().join(format!("microvid-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("out.mkv.part");
    let args = microvid_lib::command::build_args(microvid_lib::command::BuildInput {
        info: &info, settings: &settings, crop: Some(crop), external_sub: Some(&sub), output: &out, caps: &caps, clip: None,
    }).unwrap();
    let status = Command::new(&caps.ffmpeg_path).args(&args).status().unwrap();
    assert!(status.success(), "ffmpeg failed: {args:?}");
    let final_path = out_dir.join("out.mkv");
    std::fs::rename(&out, &final_path).unwrap();

    let probed = ffprobe_streams(&caps.ffprobe_path, &final_path);
    let streams = probed["streams"].as_array().unwrap();
    let video = streams.iter().find(|s| s["codec_type"] == "video").unwrap();
    assert_eq!(video["codec_name"], "hevc");
    assert_eq!(video["height"], 480);
    assert_eq!(video["width"], 1152, "1920x800 crop → 1152x480");
    let audio = streams.iter().find(|s| s["codec_type"] == "audio").unwrap();
    assert_eq!(audio["codec_name"], "aac");
    assert_eq!(audio["channels"], 2);
    let subs: Vec<_> = streams.iter().filter(|s| s["codec_type"] == "subtitle").collect();
    assert_eq!(subs.len(), 2, "embedded + external subtitle");
    assert!(subs.iter().any(|s| s["disposition"]["default"] == 1));
    let size = std::fs::metadata(&final_path).unwrap().len();
    assert!(size > 10_000 && size < info.size_bytes, "output {size} vs input {}", info.size_bytes);
    std::fs::remove_dir_all(&out_dir).ok();
}
