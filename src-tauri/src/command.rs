//! Pure function: EncodeSettings + MediaInfo → ffmpeg argv. Unit-tested, no I/O.
use crate::models::{AudioMode, AudioStream, Capabilities, Channels, Codec, Container, Crop, CropMode, EncodeSettings, MediaInfo, SubtitleMode};
use anyhow::{anyhow, Result};
use std::path::Path;

pub struct BuildInput<'a> {
    pub info: &'a MediaInfo,
    pub settings: &'a EncodeSettings,
    pub crop: Option<Crop>,
    /// External subtitle file (already delay-shifted when burning in).
    pub external_sub: Option<&'a Path>,
    /// Output file (may carry a `.part` suffix; the muxer is forced explicitly).
    pub output: &'a Path,
    pub caps: &'a Capabilities,
    /// (start_secs, duration_secs) for test encodes.
    pub clip: Option<(f64, f64)>,
    /// Hardware decoder name (`-hwaccel`), e.g. "videotoolbox"; None = software decoding.
    pub hwaccel: Option<&'a str>,
}

/// zscale + tonemap chain converting PQ/HLG BT.2020 to SDR BT.709. Input characteristics are
/// passed explicitly because many HDR/DV rips carry no colour tags.
pub fn tonemap_chain(v: &crate::models::VideoStream) -> String {
    let tin = match v.color_transfer.as_deref() {
        Some("arib-std-b67") => "arib-std-b67",
        _ => "smpte2084",
    };
    let pin = match v.color_primaries.as_deref() {
        Some(p) if p != "bt2020" && p != "unknown" => p,
        _ => "bt2020",
    };
    let min = match v.color_space.as_deref() {
        Some("bt2020c") => "bt2020c",
        _ => "bt2020nc",
    };
    // setparams stamps the tags onto every frame (zscale refuses frames with unknown colour
    // properties even when tin/pin/min are given), then zscale converts explicitly.
    format!("format=yuv420p10le,setparams=colorspace={min}:color_primaries={pin}:color_trc={tin}:range=tv,zscale=tin={tin}:pin={pin}:min={min}:t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv")
}

fn even(n: u32) -> u32 {
    n & !1
}

/// Round to the nearest even number (matches ffmpeg's `-2` behaviour: 853.3 → 854).
fn even_round(x: f64) -> u32 {
    (((x / 2.0).round() as u32) * 2).max(2)
}

/// Output width/height after crop + scale, never upscaling, aspect preserved.
pub fn output_dims(info: &MediaInfo, settings: &EncodeSettings, crop: Option<Crop>) -> (u32, u32) {
    let Some(v) = &info.video else { return (0, 0) };
    let (sw, sh) = match (settings.crop, crop) {
        (CropMode::None, _) => (v.width, v.height),
        (CropMode::Manual { w, h, .. }, _) => (w.min(v.width), h.min(v.height)),
        (CropMode::Auto, Some(c)) => (c.w, c.h),
        (CropMode::Auto, None) => (v.width, v.height),
    };
    let dar = v.dar_of(sw, sh);
    let target_h = settings.resolution.height().map(|t| t.min(sh)).unwrap_or(sh);
    let out_h = even(target_h.max(2));
    let out_w = even_round((out_h as f64) * dar);
    (out_w, out_h)
}

pub fn effective_crop(info: &MediaInfo, settings: &EncodeSettings, detected: Option<Crop>) -> Option<Crop> {
    let v = info.video.as_ref()?;
    match settings.crop {
        CropMode::None => None,
        CropMode::Manual { w, h, x, y } => Some(Crop { w: w.min(v.width), h: h.min(v.height), x, y }),
        CropMode::Auto => detected.filter(|c| c.w < v.width || c.h < v.height),
    }
}

/// Escape a path for use as a filter option value (ffmpeg's two-level escaping).
pub fn escape_filter_path(p: &str) -> String {
    let level1: String = p
        .chars()
        .flat_map(|c| if "\\'=:".contains(c) { vec!['\\', c] } else { vec![c] })
        .collect();
    level1
        .chars()
        .flat_map(|c| if "\\'[],;".contains(c) { vec!['\\', c] } else { vec![c] })
        .collect()
}

fn x264_level(h: u32, fps: f64) -> &'static str {
    if h <= 480 {
        "3.1"
    } else if h <= 720 {
        "4.0"
    } else if fps > 31.0 {
        "4.2"
    } else {
        "4.1"
    }
}

/// Which audio streams end up in the output, in output order.
pub fn select_audio<'a>(info: &'a MediaInfo, s: &EncodeSettings) -> Vec<&'a AudioStream> {
    if info.audio.is_empty() {
        return vec![];
    }
    let default_pick = || info.audio.iter().find(|x| x.is_default).or_else(|| info.audio.first()).into_iter().collect::<Vec<_>>();
    match s.audio.mode {
        AudioMode::All => info.audio.iter().collect(),
        AudioMode::Select => {
            let sel: Vec<&AudioStream> = info.audio.iter().filter(|x| s.audio.tracks.contains(&x.rel_index)).collect();
            if sel.is_empty() { default_pick() } else { sel }
        }
        AudioMode::Default => default_pick(),
    }
}

/// Dialogue-preserving stereo downmix for known surround layouts; None = let ffmpeg use `-ac 2`.
pub fn downmix_pan(layout: Option<&str>, channels: u32) -> Option<String> {
    if channels <= 2 {
        return None;
    }
    let l = layout.unwrap_or("");
    let side = l.contains("(side)");
    match l.trim_end_matches("(side)") {
        "5.1" => {
            let (sl, sr) = if side { ("SL", "SR") } else { ("BL", "BR") };
            Some(format!("pan=stereo|FL<FL+0.707*FC+0.5*{sl}|FR<FR+0.707*FC+0.5*{sr}"))
        }
        "7.1" => Some("pan=stereo|FL<FL+0.707*FC+0.5*BL+0.5*SL|FR<FR+0.707*FC+0.5*BR+0.5*SR".into()),
        _ => None,
    }
}

fn sub_codec_for_ext(ext: &str, container: Container) -> &'static str {
    match container {
        Container::Mp4 => "mov_text",
        Container::Mkv => match ext {
            "ass" | "ssa" => "copy",
            "srt" => "copy",
            _ => "srt",
        },
    }
}

pub fn build_args(input: BuildInput) -> Result<Vec<String>> {
    let BuildInput { info, settings, crop, external_sub, output, caps, clip, hwaccel } = input;
    let v = info.video.as_ref().ok_or_else(|| anyhow!("source has no video stream"))?;
    let s = settings;
    let mut a: Vec<String> = vec![];
    let push = |a: &mut Vec<String>, items: &[&str]| a.extend(items.iter().map(|x| x.to_string()));

    push(&mut a, &["-y", "-hide_banner", "-nostdin", "-loglevel", "error", "-nostats", "-progress", "pipe:1", "-stats_period", "0.5"]);

    // ---- inputs ----
    if let Some(hw) = hwaccel {
        // Frames are downloaded to system memory automatically, so software filters still apply;
        // ffmpeg falls back to software decoding if the hardware path fails.
        a.extend(["-hwaccel".into(), hw.into()]);
    }
    if let Some((start, dur)) = clip {
        a.extend(["-ss".into(), format!("{start:.3}"), "-t".into(), format!("{dur:.3}")]);
    }
    a.extend(["-i".into(), info.path.clone()]);
    let mux_external = external_sub.is_some() && !s.subtitles.burn_in && clip.is_none();
    if mux_external {
        let sub = external_sub.unwrap();
        if s.subtitles.delay_ms != 0 {
            a.extend(["-itsoffset".into(), format!("{:.3}", s.subtitles.delay_ms as f64 / 1000.0)]);
        }
        a.extend(["-i".into(), sub.to_string_lossy().into_owned()]);
    }

    // ---- maps ----
    // Map the video stream by absolute index so embedded cover art (an attached picture that
    // may be the first video stream) is never picked up.
    a.extend(["-map".into(), format!("0:{}", v.index)]);
    let mapped_audio: Vec<&AudioStream> = select_audio(info, s);
    for t in &mapped_audio {
        a.extend(["-map".into(), format!("0:a:{}", t.rel_index)]);
    }
    // Source subtitle streams (text-only for mp4).
    let mut sub_out_index = 0usize;
    let mut sub_codec_args: Vec<String> = vec![];
    let keep_source_subs = s.subtitles.keep_source_subs && s.subtitles.mode != SubtitleMode::None && clip.is_none();
    // (output subtitle index, source rel index) for source tracks; external gets None.
    let mut sub_outputs: Vec<Option<usize>> = vec![];
    if keep_source_subs {
        for st in &info.subtitles {
            if s.container == Container::Mp4 && !st.text_based {
                continue;
            }
            if let Some(sel) = &s.subtitles.source_tracks {
                if !sel.contains(&st.rel_index) {
                    continue;
                }
            }
            a.extend(["-map".into(), format!("0:s:{}", st.rel_index)]);
            let codec = if s.container == Container::Mp4 { "mov_text" } else { "copy" };
            sub_codec_args.extend([format!("-c:s:{sub_out_index}"), codec.into()]);
            sub_outputs.push(Some(st.rel_index));
            sub_out_index += 1;
        }
    }
    if mux_external {
        let sub = external_sub.unwrap();
        let ext = sub.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        a.extend(["-map".into(), "1:0".into()]);
        sub_codec_args.extend([format!("-c:s:{sub_out_index}"), sub_codec_for_ext(&ext, s.container).into()]);
        sub_codec_args.extend([format!("-metadata:s:s:{sub_out_index}"), format!("language={}", s.subtitles.language)]);
        sub_outputs.push(None);
        sub_out_index += 1;
    }
    // Default subtitle flag: explicit choice, else the external file when there is one.
    let default_sub: Option<usize> = match s.subtitles.default_track {
        Some(-1) => sub_outputs.iter().position(|o| o.is_none()),
        Some(n) if n >= 0 => sub_outputs.iter().position(|o| *o == Some(n as usize)),
        _ => sub_outputs.iter().position(|o| o.is_none()),
    };
    if let Some(d) = default_sub {
        for (k, _) in sub_outputs.iter().enumerate() {
            sub_codec_args.extend([format!("-disposition:s:{k}"), if k == d { "default".into() } else { "0".into() }]);
        }
    }
    let _ = sub_out_index;
    if clip.is_none() {
        push(&mut a, &["-map_metadata", "0"]);
        if s.container == Container::Mkv {
            push(&mut a, &["-map_chapters", "0"]);
        } else {
            push(&mut a, &["-map_chapters", "-1"]);
        }
    }

    // ---- video filters ----
    let (out_w, out_h) = output_dims(info, s, crop);
    let mut vf: Vec<String> = vec![];
    if let Some(c) = effective_crop(info, s, crop) {
        vf.push(format!("crop={}:{}:{}:{}", c.w, c.h, c.x, c.y));
    }
    let needs_scale = out_w != v.width || out_h != v.height || (v.sar - 1.0).abs() > 1e-3 || vf.iter().any(|f| f.starts_with("crop"));
    if needs_scale {
        vf.push(format!("scale={out_w}:{out_h}:flags=lanczos"));
        vf.push("setsar=1".into());
    }
    let tonemap = v.hdr && s.tonemap_hdr;
    if tonemap {
        // After scaling (cheaper), before subtitles (so text is rendered in SDR).
        vf.push(tonemap_chain(v));
    }
    if s.subtitles.burn_in && clip.is_none() {
        if let Some(sub) = external_sub {
            vf.push(format!("subtitles={}", escape_filter_path(&sub.to_string_lossy())));
        } else if let Some(st) = info.subtitles.iter().find(|x| x.text_based) {
            vf.push(format!("subtitles={}:si={}", escape_filter_path(&info.path), st.rel_index));
        }
    }
    let crf = s.effective_crf();
    let preset = s.effective_preset();
    let ten_bit = s.ten_bit || s.codec == Codec::Av1;
    let pix_fmt = if ten_bit { "yuv420p10le" } else { "yuv420p" };
    if tonemap {
        vf.push(format!("format={pix_fmt}"));
    }
    if !vf.is_empty() {
        a.extend(["-vf".into(), vf.join(",")]);
    }

    // ---- video encoder ----
    let hw = if s.hardware {
        match s.codec {
            Codec::X264 => caps.hw_h264.clone(),
            Codec::Hevc => caps.hw_hevc.clone(),
            Codec::Av1 => None,
        }
    } else {
        None
    };
    if let Some(enc) = hw {
        a.extend(["-c:v".into(), enc.clone()]);
        let q = (100u32.saturating_sub(crf as u32 * 2)).clamp(30, 90);
        if enc.ends_with("videotoolbox") {
            a.extend(["-q:v".into(), q.to_string(), "-allow_sw".into(), "1".into()]);
        } else if enc.ends_with("nvenc") {
            a.extend(["-rc".into(), "vbr".into(), "-cq".into(), crf.to_string(), "-b:v".into(), "0".into(), "-preset".into(), "p6".into()]);
        } else if enc.ends_with("qsv") {
            a.extend(["-global_quality".into(), crf.to_string(), "-preset".into(), "slower".into()]);
        } else if enc.ends_with("amf") {
            a.extend(["-rc".into(), "cqp".into(), "-qp_i".into(), crf.to_string(), "-qp_p".into(), crf.to_string()]);
        }
        a.extend(["-pix_fmt".into(), if s.codec == Codec::Hevc && s.ten_bit { "p010le".into() } else { "yuv420p".into() }]);
        if s.codec == Codec::Hevc {
            push(&mut a, &["-tag:v", "hvc1"]);
        }
    } else {
        match s.codec {
            Codec::X264 => {
                a.extend(["-c:v".into(), "libx264".into(), "-preset".into(), preset, "-crf".into(), crf.to_string()]);
                a.extend(["-profile:v".into(), if ten_bit { "high10".into() } else { "main".into() }]);
                a.extend(["-level".into(), x264_level(out_h, v.fps).into()]);
                let tune = s.tune.clone().or_else(|| match s.content_type {
                    crate::models::ContentType::Drama => Some("film".into()),
                    crate::models::ContentType::Animation => Some("animation".into()),
                    _ => None,
                });
                if let Some(t) = tune.filter(|t| !t.is_empty() && t != "none") {
                    a.extend(["-tune".into(), t]);
                }
                a.extend(["-pix_fmt".into(), pix_fmt.into()]);
            }
            Codec::Hevc => {
                a.extend(["-c:v".into(), "libx265".into(), "-preset".into(), preset, "-crf".into(), crf.to_string()]);
                a.extend(["-profile:v".into(), if ten_bit { "main10".into() } else { "main".into() }]);
                let tune = s.tune.clone().or_else(|| match s.content_type {
                    crate::models::ContentType::Animation => Some("animation".into()),
                    _ => None,
                });
                if let Some(t) = tune.filter(|t| !t.is_empty() && t != "none") {
                    a.extend(["-tune".into(), t]);
                }
                a.extend(["-pix_fmt".into(), pix_fmt.into(), "-tag:v".into(), "hvc1".into()]);
                a.extend(["-x265-params".into(), "log-level=error".into()]);
            }
            Codec::Av1 => {
                a.extend(["-c:v".into(), "libsvtav1".into(), "-preset".into(), preset, "-crf".into(), crf.to_string()]);
                a.extend(["-svtav1-params".into(), "tune=0:film-grain=0".into(), "-pix_fmt".into(), "yuv420p10le".into()]);
            }
        }
    }

    // ---- audio ----
    if mapped_audio.is_empty() {
        push(&mut a, &["-an"]);
    } else {
        a.extend(["-c:a".into(), caps.audio_encoder().into(), "-b:a".into(), format!("{}k", s.audio.bitrate_kbps)]);
        let default_idx = s.audio.default_track.and_then(|d| mapped_audio.iter().position(|t| t.rel_index == d)).unwrap_or(0);
        for n in 0..mapped_audio.len() {
            a.extend([format!("-disposition:a:{n}"), if n == default_idx { "default".into() } else { "0".into() }]);
        }
        match s.audio.channels {
            Channels::Mono => push(&mut a, &["-ac", "1"]),
            Channels::Stereo => {
                let mut need_generic = false;
                for (n, t) in mapped_audio.iter().enumerate() {
                    if t.channels > 2 {
                        if let Some(pan) = downmix_pan(t.channel_layout.as_deref(), t.channels) {
                            a.extend([format!("-filter:a:{n}"), pan]);
                        } else {
                            need_generic = true;
                        }
                    }
                }
                if need_generic {
                    push(&mut a, &["-ac", "2"]);
                }
            }
        }
    }

    // ---- subtitles codec/meta ----
    if sub_codec_args.is_empty() && keep_source_subs {
        // nothing mapped
    }
    a.extend(sub_codec_args);
    if !keep_source_subs && !mux_external {
        push(&mut a, &["-sn"]);
    }

    // ---- container ----
    match s.container {
        Container::Mkv => push(&mut a, &["-f", "matroska"]),
        Container::Mp4 => push(&mut a, &["-f", "mp4", "-movflags", "+faststart"]),
    }
    push(&mut a, &["-max_muxing_queue_size", "2048"]);
    a.extend(s.extra_args.iter().filter(|x| !x.trim().is_empty()).cloned());
    a.push(output.to_string_lossy().into_owned());
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn info(w: u32, h: u32, sar: f64) -> MediaInfo {
        MediaInfo {
            path: "/media/Show S01E01.mkv".into(),
            size_bytes: 1_400_000_000,
            duration_secs: 2600.0,
            container: "matroska".into(),
            bitrate: None,
            video: Some(VideoStream {
                index: 0, codec: "h264".into(), profile: None, width: w, height: h, fps: 23.976, sar,
                pix_fmt: "yuv420p".into(), bit_depth: 8, bitrate: None, hdr: false, color_transfer: None, color_primaries: None, color_space: None,
            }),
            audio: vec![AudioStream {
                index: 1, rel_index: 0, codec: "ac3".into(), channels: 6, channel_layout: Some("5.1(side)".into()),
                language: Some("eng".into()), title: None, bitrate: Some(384000), is_default: true,
            }],
            subtitles: vec![SubtitleStream {
                index: 2, rel_index: 0, codec: "subrip".into(), language: Some("eng".into()), title: None,
                forced: false, is_default: false, text_based: true,
            }],
            chapters: 3,
        }
    }
    fn caps() -> Capabilities {
        Capabilities {
            ffmpeg_path: "ffmpeg".into(), ffprobe_path: "ffprobe".into(), source: "bundled".into(), version: "9".into(),
            encoders: vec![], has_aac_at: true, has_fdk_aac: false, has_x264: true, has_x265: true, has_svtav1: true,
            hw_h264: Some("h264_videotoolbox".into()), hw_hevc: Some("hevc_videotoolbox".into()), hwaccels: vec!["videotoolbox".into()],
        }
    }
    fn args(info: &MediaInfo, s: &EncodeSettings, crop: Option<Crop>, sub: Option<&Path>) -> Vec<String> {
        build_args(BuildInput { info, settings: s, crop, external_sub: sub, output: Path::new("/out/x.mkv.part"), caps: &caps(), clip: None, hwaccel: None }).unwrap()
    }
    fn has_pair(a: &[String], k: &str, v: &str) -> bool {
        a.windows(2).any(|w| w[0] == k && w[1] == v)
    }

    #[test]
    fn default_hevc_480p() {
        let i = info(1920, 1080, 1.0);
        let s = EncodeSettings::default();
        let a = args(&i, &s, None, None);
        assert!(has_pair(&a, "-c:v", "libx265"));
        assert!(has_pair(&a, "-preset", "slow"));
        assert!(has_pair(&a, "-crf", "26"));
        assert!(has_pair(&a, "-tag:v", "hvc1"));
        assert!(has_pair(&a, "-vf", "scale=854:480:flags=lanczos,setsar=1"));
        assert!(has_pair(&a, "-c:a", "aac_at"));
        assert!(has_pair(&a, "-b:a", "80k"));
        assert!(a.iter().any(|x| x.starts_with("pan=stereo|FL<FL+0.707*FC+0.5*SL")));
        assert!(has_pair(&a, "-map", "0:0"), "video mapped by absolute index");
        assert!(has_pair(&a, "-map", "0:s:0"));
        assert!(has_pair(&a, "-c:s:0", "copy"));
        assert!(has_pair(&a, "-disposition:a:0", "default"));
        assert!(has_pair(&a, "-f", "matroska"));
        assert_eq!(a.last().unwrap(), "/out/x.mkv.part");
    }

    #[test]
    fn never_upscales_and_keeps_aspect() {
        let i = info(1280, 720, 1.0);
        let mut s = EncodeSettings::default();
        s.resolution = Resolution::P1080;
        assert_eq!(output_dims(&i, &s, None), (1280, 720));
        let a = args(&i, &s, None, None);
        assert!(!a.iter().any(|x| x.starts_with("scale=")), "no scale filter expected: {a:?}");
        // anamorphic 1440x1080 with SAR 4:3 → 16:9 output
        let i2 = info(1440, 1080, 4.0 / 3.0);
        s.resolution = Resolution::P480;
        assert_eq!(output_dims(&i2, &s, None), (854, 480));
    }

    #[test]
    fn crop_then_scale() {
        let i = info(1920, 1080, 1.0);
        let s = EncodeSettings::default();
        let c = Crop { w: 1920, h: 800, x: 0, y: 140 };
        assert_eq!(output_dims(&i, &s, Some(c)), (1152, 480));
        let a = args(&i, &s, Some(c), None);
        assert!(has_pair(&a, "-vf", "crop=1920:800:0:140,scale=1152:480:flags=lanczos,setsar=1"));
    }

    #[test]
    fn x264_recipe() {
        let i = info(1920, 1080, 1.0);
        let mut s = EncodeSettings::default();
        s.codec = Codec::X264;
        s.resolution = Resolution::P360;
        s.content_type = ContentType::Drama;
        let a = args(&i, &s, None, None);
        assert!(has_pair(&a, "-c:v", "libx264"));
        assert!(has_pair(&a, "-preset", "veryslow"));
        assert!(has_pair(&a, "-crf", "24"));
        assert!(has_pair(&a, "-profile:v", "main"));
        assert!(has_pair(&a, "-level", "3.1"));
        assert!(has_pair(&a, "-tune", "film"));
        assert!(has_pair(&a, "-vf", "scale=640:360:flags=lanczos,setsar=1"));
    }

    #[test]
    fn external_subtitle_with_delay() {
        let i = info(1920, 1080, 1.0);
        let mut s = EncodeSettings::default();
        s.subtitles.mode = SubtitleMode::File;
        s.subtitles.delay_ms = -1500;
        s.subtitles.keep_source_subs = false;
        let a = args(&i, &s, None, Some(Path::new("/media/Show S01E01.en.srt")));
        assert!(has_pair(&a, "-itsoffset", "-1.500"));
        assert!(has_pair(&a, "-i", "/media/Show S01E01.en.srt"));
        assert!(has_pair(&a, "-map", "1:0"));
        assert!(!has_pair(&a, "-map", "0:s:0"));
        assert!(has_pair(&a, "-c:s:0", "copy"));
        assert!(has_pair(&a, "-metadata:s:s:0", "language=eng"));
        assert!(has_pair(&a, "-disposition:s:0", "default"));
    }

    #[test]
    fn burn_in_uses_filter() {
        let i = info(1920, 1080, 1.0);
        let mut s = EncodeSettings::default();
        s.subtitles.mode = SubtitleMode::File;
        s.subtitles.burn_in = true;
        let a = args(&i, &s, None, Some(Path::new("/m/it's.srt")));
        let vf = a.iter().position(|x| x == "-vf").map(|p| a[p + 1].clone()).unwrap();
        assert!(vf.ends_with("subtitles=/m/it\\\\\\'s.srt"), "{vf}");
        assert!(!has_pair(&a, "-map", "1:0"));
    }

    #[test]
    fn av1_and_mono_and_mp4() {
        let i = info(1920, 1080, 1.0);
        let mut s = EncodeSettings::default();
        s.codec = Codec::Av1;
        s.content_type = ContentType::News;
        s.audio.channels = Channels::Mono;
        s.audio.bitrate_kbps = 48;
        s.container = Container::Mp4;
        let a = args(&i, &s, None, None);
        assert!(has_pair(&a, "-c:v", "libsvtav1"));
        assert!(has_pair(&a, "-crf", "40"));
        assert!(has_pair(&a, "-pix_fmt", "yuv420p10le"));
        assert!(has_pair(&a, "-ac", "1"));
        assert!(has_pair(&a, "-c:s:0", "mov_text"));
        assert!(has_pair(&a, "-f", "mp4"));
    }

    #[test]
    fn hardware_fast_mode() {
        let i = info(1920, 1080, 1.0);
        let mut s = EncodeSettings::default();
        s.hardware = true;
        let a = args(&i, &s, None, None);
        assert!(has_pair(&a, "-c:v", "hevc_videotoolbox"));
        assert!(has_pair(&a, "-q:v", "48"));
    }

    #[test]
    fn clip_mode_skips_subs() {
        let i = info(1920, 1080, 1.0);
        let s = EncodeSettings::default();
        let a = build_args(BuildInput { info: &i, settings: &s, crop: None, external_sub: Some(Path::new("/x.srt")), output: Path::new("/o.mkv"), caps: &caps(), clip: Some((1300.0, 30.0)), hwaccel: None }).unwrap();
        assert!(has_pair(&a, "-ss", "1300.000"));
        assert!(has_pair(&a, "-t", "30.000"));
        assert!(!has_pair(&a, "-map", "1:0"));
        assert!(!has_pair(&a, "-map", "0:s:0"));
    }

    #[test]
    fn multiple_audio_tracks_and_default_choice() {
        let mut i = info(1920, 1080, 1.0);
        i.audio.push(AudioStream { index: 3, rel_index: 1, codec: "aac".into(), channels: 2, channel_layout: Some("stereo".into()), language: Some("hin".into()), title: None, bitrate: None, is_default: false });
        i.subtitles.push(SubtitleStream { index: 4, rel_index: 1, codec: "ass".into(), language: Some("hin".into()), title: None, forced: false, is_default: false, text_based: true });
        // keep all audio, make the second one default
        let mut s = EncodeSettings::default();
        s.audio.mode = AudioMode::All;
        s.audio.default_track = Some(1);
        let a = args(&i, &s, None, None);
        assert!(has_pair(&a, "-map", "0:a:0") && has_pair(&a, "-map", "0:a:1"));
        assert!(has_pair(&a, "-disposition:a:0", "0"));
        assert!(has_pair(&a, "-disposition:a:1", "default"));
        // select only the second audio, keep only subtitle s:1 and make it default
        let mut s2 = EncodeSettings::default();
        s2.audio.mode = AudioMode::Select;
        s2.audio.tracks = vec![1];
        s2.subtitles.source_tracks = Some(vec![1]);
        s2.subtitles.default_track = Some(1);
        let a2 = args(&i, &s2, None, None);
        assert!(!has_pair(&a2, "-map", "0:a:0") && has_pair(&a2, "-map", "0:a:1"));
        assert!(!has_pair(&a2, "-map", "0:s:0") && has_pair(&a2, "-map", "0:s:1"));
        assert!(has_pair(&a2, "-disposition:s:0", "default"));
        // an invalid selection falls back to the default track rather than dropping audio
        let mut s3 = EncodeSettings::default();
        s3.audio.mode = AudioMode::Select;
        s3.audio.tracks = vec![9];
        let a3 = args(&i, &s3, None, None);
        assert!(has_pair(&a3, "-map", "0:a:0"));
    }

    #[test]
    fn hdr_source_is_tonemapped_and_hw_decoded() {
        let mut i = info(3840, 1608, 1.0);
        let v = i.video.as_mut().unwrap();
        v.hdr = true;
        v.bit_depth = 10;
        v.pix_fmt = "yuv420p10le".into();
        let s = EncodeSettings::default();
        let a = build_args(BuildInput { info: &i, settings: &s, crop: None, external_sub: None, output: Path::new("/o.mkv"), caps: &caps(), clip: None, hwaccel: Some("videotoolbox") }).unwrap();
        assert_eq!(&a[a.iter().position(|x| x == "-hwaccel").unwrap() + 1], "videotoolbox");
        assert!(a.iter().position(|x| x == "-hwaccel").unwrap() < a.iter().position(|x| x == "-i").unwrap());
        let vf = a.iter().position(|x| x == "-vf").map(|p| a[p + 1].clone()).unwrap();
        assert!(vf.starts_with("scale=1146:480:flags=lanczos,setsar=1,format=yuv420p10le,setparams=colorspace=bt2020nc:color_primaries=bt2020:color_trc=smpte2084:range=tv,zscale=tin=smpte2084:pin=bt2020:min=bt2020nc:t=linear"), "{vf}");
        assert!(vf.contains("tonemap=tonemap=hable"));
        assert!(vf.ends_with("format=yuv420p"), "{vf}");
        let mut s2 = EncodeSettings::default();
        s2.tonemap_hdr = false;
        let a2 = args(&i, &s2, None, None);
        assert!(!a2.iter().any(|x| x.contains("tonemap")));
    }

    #[test]
    fn cover_art_is_not_the_video() {
        let mut i = info(1920, 1080, 1.0);
        i.video.as_mut().unwrap().index = 1; // stream 0 is an attached picture
        let a = args(&i, &EncodeSettings::default(), None, None);
        assert!(has_pair(&a, "-map", "0:1"));
        assert!(!has_pair(&a, "-map", "0:v:0"));
    }

    #[test]
    fn downmix_layouts() {
        assert!(downmix_pan(Some("5.1"), 6).unwrap().contains("0.5*BL"));
        assert!(downmix_pan(Some("5.1(side)"), 6).unwrap().contains("0.5*SL"));
        assert!(downmix_pan(Some("7.1"), 8).unwrap().contains("SL"));
        assert!(downmix_pan(Some("stereo"), 2).is_none());
        assert!(downmix_pan(Some("4.0"), 4).is_none());
    }
}
