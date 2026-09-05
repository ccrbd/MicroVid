//! Output size and encode time estimation. Table-driven, then calibrated from real jobs.
use crate::command::output_dims;
use crate::models::{Codec, Crop, EncodeSettings, Estimate, MediaInfo};

/// Baseline video kbps for "general" content at the table CRF (24 fps).
fn base_kbps(codec: Codec, out_h: u32) -> f64 {
    let bucket = height_bucket(out_h);
    let t: [(u32, f64); 5] = match codec {
        Codec::X264 => [(360, 450.0), (480, 700.0), (576, 900.0), (720, 1300.0), (1080, 2600.0)],
        Codec::Hevc => [(360, 300.0), (480, 450.0), (576, 580.0), (720, 850.0), (1080, 1700.0)],
        Codec::Av1 => [(360, 250.0), (480, 370.0), (576, 480.0), (720, 700.0), (1080, 1400.0)],
    };
    t.iter().find(|(h, _)| *h == bucket).map(|(_, k)| *k).unwrap_or(t[4].1)
}

/// Baseline software-encode throughput (frames/s) on an ~8 physical core desktop.
fn base_fps(codec: Codec, out_h: u32, hardware: bool) -> f64 {
    let bucket = height_bucket(out_h);
    if hardware {
        return match bucket {
            360 | 480 => 400.0,
            576 | 720 => 300.0,
            _ => 150.0,
        };
    }
    let t: [(u32, f64); 5] = match codec {
        Codec::X264 => [(360, 120.0), (480, 80.0), (576, 65.0), (720, 45.0), (1080, 22.0)],
        Codec::Hevc => [(360, 90.0), (480, 60.0), (576, 48.0), (720, 33.0), (1080, 15.0)],
        Codec::Av1 => [(360, 45.0), (480, 30.0), (576, 24.0), (720, 16.0), (1080, 7.0)],
    };
    t.iter().find(|(h, _)| *h == bucket).map(|(_, k)| *k).unwrap_or(t[4].1)
}

pub fn height_bucket(h: u32) -> u32 {
    if h <= 400 {
        360
    } else if h <= 520 {
        480
    } else if h <= 640 {
        576
    } else if h <= 900 {
        720
    } else {
        1080
    }
}

/// Speed multiplier of a preset relative to the default preset of that codec.
pub fn preset_speed_factor(codec: Codec, preset: &str) -> f64 {
    match codec {
        Codec::X264 => match preset {
            "ultrafast" => 12.0,
            "superfast" => 9.0,
            "veryfast" => 6.0,
            "faster" => 4.8,
            "fast" => 4.0,
            "medium" => 3.2,
            "slow" => 2.2,
            "slower" => 1.4,
            _ => 1.0, // veryslow
        },
        Codec::Hevc => match preset {
            "ultrafast" => 5.5,
            "superfast" => 4.0,
            "veryfast" => 3.0,
            "faster" => 2.4,
            "fast" => 2.0,
            "medium" => 1.5,
            "slower" => 0.65,
            "veryslow" => 0.4,
            _ => 1.0, // slow
        },
        Codec::Av1 => {
            let p: f64 = preset.parse().unwrap_or(4.0);
            2f64.powf((p - 4.0) / 1.5)
        }
    }
}

/// Size effect of moving away from the table CRF: roughly halves every +6 CRF.
fn crf_factor(codec: Codec, crf: u8, base: u8) -> f64 {
    let per_step: f64 = match codec {
        Codec::X264 | Codec::Hevc => 0.891,
        Codec::Av1 => 0.925,
    };
    per_step.powi(crf as i32 - base as i32)
}

/// Calibration key used for stored throughput measurements.
pub fn calibration_key(settings: &EncodeSettings, out_h: u32) -> String {
    let enc = if settings.hardware { "hw" } else { settings.codec.key() };
    format!("{enc}:{}:{}", settings.effective_preset(), height_bucket(out_h))
}

pub struct EstimateContext {
    /// Machine throughput (frames/s across all parallel jobs) measured for this calibration key.
    pub calibrated_throughput: Option<f64>,
    pub physical_cores: u32,
    pub parallel_jobs: u32,
}

pub fn estimate(info: &MediaInfo, settings: &EncodeSettings, crop: Option<Crop>, ctx: &EstimateContext) -> Estimate {
    let (out_w, out_h) = output_dims(info, settings, crop);
    let v = info.video.as_ref();
    let fps = v.map(|v| v.fps).unwrap_or(24.0);
    let crf = settings.effective_crf();
    let base_crf = settings.content_type.default_crf(settings.codec);

    let mut video_kbps = base_kbps(settings.codec, out_h)
        * settings.content_type.size_multiplier()
        * crf_factor(settings.codec, crf, base_crf)
        * (fps / 24.0).sqrt().clamp(0.7, 1.8);
    // Scale by actual pixel count vs the bucket's 16:9 reference so 4:3 or scope crops are honoured.
    let ref_px = (height_bucket(out_h) as f64) * (height_bucket(out_h) as f64) * 16.0 / 9.0;
    let px = (out_w as f64) * (out_h as f64);
    if ref_px > 0.0 {
        video_kbps *= (px / ref_px).powf(0.85);
    }
    if settings.hardware {
        video_kbps *= 1.45;
    }
    if settings.ten_bit && settings.codec != Codec::Av1 {
        video_kbps *= 0.95;
    }
    let audio_tracks = crate::command::select_audio(info, settings).len() as f64;
    let audio_kbps = settings.audio.bitrate_kbps as f64 * audio_tracks;
    let size_bytes = ((video_kbps + audio_kbps) * 1000.0 / 8.0 * info.duration_secs) as u64;

    let frames = info.duration_secs * fps;
    let preset_factor = preset_speed_factor(settings.codec, &settings.effective_preset());
    let core_factor = ((ctx.physical_cores.max(1) as f64) / 8.0).powf(0.7).clamp(0.3, 3.0);
    let (throughput, calibrated) = match ctx.calibrated_throughput {
        Some(t) if t > 0.0 => (t, true),
        _ => (base_fps(settings.codec, out_h, settings.hardware) * preset_factor * core_factor, false),
    };
    let per_job_fps = throughput / (ctx.parallel_jobs.max(1) as f64);
    let seconds = frames / per_job_fps.max(0.1) * 1.03;

    let note = match (v, settings.resolution.height()) {
        (Some(v), Some(h)) if v.height < h => Some(format!("Source is {}p, output stays {}p (no upscaling)", v.height, v.height)),
        _ => None,
    };
    Estimate {
        size_bytes,
        seconds,
        out_width: out_w,
        out_height: out_h,
        crf,
        video_kbps: video_kbps.round() as u32,
        audio_kbps: audio_kbps.round() as u32,
        fps_assumed: per_job_fps,
        calibrated,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn info() -> MediaInfo {
        MediaInfo {
            path: "x".into(), size_bytes: 1_400_000_000, duration_secs: 3600.0, container: "mkv".into(), bitrate: None,
            video: Some(VideoStream { index: 0, codec: "h264".into(), profile: None, width: 1920, height: 1080, fps: 24.0, sar: 1.0, pix_fmt: "yuv420p".into(), bit_depth: 8, bitrate: None, hdr: false }),
            audio: vec![AudioStream { index: 1, rel_index: 0, codec: "aac".into(), channels: 2, channel_layout: None, language: None, title: None, bitrate: None, is_default: true }],
            subtitles: vec![], chapters: 0,
        }
    }

    #[test]
    fn hevc_480p_hour_is_a_couple_hundred_mb() {
        let e = estimate(&info(), &EncodeSettings::default(), None, &EstimateContext { calibrated_throughput: None, physical_cores: 8, parallel_jobs: 1 });
        let mb = e.size_bytes as f64 / 1_048_576.0;
        assert!(mb > 150.0 && mb < 320.0, "{mb}");
        assert_eq!((e.out_width, e.out_height), (854, 480));
        assert!(e.seconds > 600.0 && e.seconds < 7200.0, "{}", e.seconds);
        assert!(!e.calibrated);
    }

    #[test]
    fn calibration_and_parallel_jobs_change_time() {
        let ctx1 = EstimateContext { calibrated_throughput: Some(100.0), physical_cores: 8, parallel_jobs: 1 };
        let ctx2 = EstimateContext { calibrated_throughput: Some(100.0), physical_cores: 8, parallel_jobs: 2 };
        let e1 = estimate(&info(), &EncodeSettings::default(), None, &ctx1);
        let e2 = estimate(&info(), &EncodeSettings::default(), None, &ctx2);
        assert!(e1.calibrated);
        assert!((e2.seconds / e1.seconds - 2.0).abs() < 0.01);
    }

    #[test]
    fn higher_crf_is_smaller_and_animation_is_smaller() {
        let base = estimate(&info(), &EncodeSettings::default(), None, &EstimateContext { calibrated_throughput: None, physical_cores: 8, parallel_jobs: 1 });
        let mut s = EncodeSettings::default();
        s.crf = Some(32);
        let hi = estimate(&info(), &s, None, &EstimateContext { calibrated_throughput: None, physical_cores: 8, parallel_jobs: 1 });
        assert!(hi.size_bytes < base.size_bytes);
        let mut s2 = EncodeSettings::default();
        s2.content_type = ContentType::Animation;
        let anim = estimate(&info(), &s2, None, &EstimateContext { calibrated_throughput: None, physical_cores: 8, parallel_jobs: 1 });
        assert!(anim.size_bytes < base.size_bytes);
    }

    #[test]
    fn upscale_note() {
        let mut i = info();
        i.video.as_mut().unwrap().height = 720;
        i.video.as_mut().unwrap().width = 1280;
        let mut s = EncodeSettings::default();
        s.resolution = Resolution::P1080;
        let e = estimate(&i, &s, None, &EstimateContext { calibrated_throughput: None, physical_cores: 8, parallel_jobs: 1 });
        assert_eq!(e.out_height, 720);
        assert!(e.note.unwrap().contains("no upscaling"));
    }
}
