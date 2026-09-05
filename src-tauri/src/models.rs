//! Shared data types exchanged between the Rust backend and the React frontend.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    X264,
    Hevc,
    Av1,
}

impl Codec {
    pub fn label(self) -> &'static str {
        match self {
            Codec::X264 => "x264",
            Codec::Hevc => "HEVC",
            Codec::Av1 => "AV1",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Codec::X264 => "x264",
            Codec::Hevc => "hevc",
            Codec::Av1 => "av1",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Resolution {
    #[serde(rename = "360")]
    P360,
    #[serde(rename = "480")]
    P480,
    #[serde(rename = "576")]
    P576,
    #[serde(rename = "720")]
    P720,
    #[serde(rename = "1080")]
    P1080,
    #[serde(rename = "source")]
    Source,
}

impl Resolution {
    /// Target height, or None to keep the source height.
    pub fn height(self) -> Option<u32> {
        match self {
            Resolution::P360 => Some(360),
            Resolution::P480 => Some(480),
            Resolution::P576 => Some(576),
            Resolution::P720 => Some(720),
            Resolution::P1080 => Some(1080),
            Resolution::Source => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    General,
    Drama,
    Sitcom,
    Animation,
    Action,
    News,
}

impl ContentType {
    /// Default CRF per codec, generalised from the "threesixtyp" recipe.
    pub fn default_crf(self, codec: Codec) -> u8 {
        match (codec, self) {
            (Codec::X264, ContentType::General) => 24,
            (Codec::X264, ContentType::Drama) => 24,
            (Codec::X264, ContentType::Sitcom) => 25,
            (Codec::X264, ContentType::Animation) => 27,
            (Codec::X264, ContentType::Action) => 22,
            (Codec::X264, ContentType::News) => 28,
            (Codec::Hevc, ContentType::General) => 26,
            (Codec::Hevc, ContentType::Drama) => 26,
            (Codec::Hevc, ContentType::Sitcom) => 27,
            (Codec::Hevc, ContentType::Animation) => 29,
            (Codec::Hevc, ContentType::Action) => 24,
            (Codec::Hevc, ContentType::News) => 30,
            (Codec::Av1, ContentType::General) => 34,
            (Codec::Av1, ContentType::Drama) => 34,
            (Codec::Av1, ContentType::Sitcom) => 36,
            (Codec::Av1, ContentType::Animation) => 38,
            (Codec::Av1, ContentType::Action) => 32,
            (Codec::Av1, ContentType::News) => 40,
        }
    }
    /// Relative source complexity at equal CRF, used by the size estimator.
    pub fn size_multiplier(self) -> f64 {
        match self {
            ContentType::General => 1.0,
            ContentType::Drama => 1.05,
            ContentType::Sitcom => 0.9,
            ContentType::Animation => 0.65,
            ContentType::Action => 1.4,
            ContentType::News => 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channels {
    Stereo,
    Mono,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioMode {
    /// The source's default track only.
    Default,
    All,
    /// The tracks listed in `tracks`.
    Select,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AudioSettings {
    pub bitrate_kbps: u32,
    pub channels: Channels,
    pub mode: AudioMode,
    /// Audio stream indexes (a:N) to keep when mode is Select.
    pub tracks: Vec<usize>,
    /// Which kept track is flagged as default (a:N). None = the first kept track.
    pub default_track: Option<usize>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { bitrate_kbps: 80, channels: Channels::Stereo, mode: AudioMode::Default, tracks: vec![], default_track: None }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum CropMode {
    Auto,
    None,
    Manual { w: u32, h: u32, x: u32, y: u32 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleMode {
    /// Best matching file next to the video, else source tracks only.
    Auto,
    /// A specific external file (settings.subtitles.file).
    File,
    /// Only keep subtitle tracks already inside the source.
    Source,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SubtitleSettings {
    pub mode: SubtitleMode,
    pub file: Option<String>,
    pub delay_ms: i64,
    pub burn_in: bool,
    pub keep_source_subs: bool,
    /// Source subtitle streams (s:N) to keep; None = all of them.
    pub source_tracks: Option<Vec<usize>>,
    /// Track flagged default: -1 = the external file, N = source stream s:N, None = auto.
    pub default_track: Option<i64>,
    /// ISO 639-2 language tag applied to an external subtitle track.
    pub language: String,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            mode: SubtitleMode::Auto,
            file: None,
            delay_ms: 0,
            burn_in: false,
            keep_source_subs: true,
            source_tracks: None,
            default_track: None,
            language: "eng".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    Mkv,
    Mp4,
}

impl Container {
    pub fn ext(self) -> &'static str {
        match self {
            Container::Mkv => "mkv",
            Container::Mp4 => "mp4",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EncodeSettings {
    pub codec: Codec,
    pub resolution: Resolution,
    pub content_type: ContentType,
    /// None = use the content-type table.
    pub crf: Option<u8>,
    /// Encoder preset override (x264/x265 names, or "0".."13" for SVT-AV1).
    pub preset: Option<String>,
    pub ten_bit: bool,
    pub tune: Option<String>,
    /// Use a hardware encoder ("fast mode") instead of the software encoder.
    pub hardware: bool,
    pub audio: AudioSettings,
    pub crop: CropMode,
    pub subtitles: SubtitleSettings,
    pub container: Container,
    pub extra_args: Vec<String>,
}

impl Default for EncodeSettings {
    fn default() -> Self {
        Self {
            codec: Codec::Hevc,
            resolution: Resolution::P480,
            content_type: ContentType::General,
            crf: None,
            preset: None,
            ten_bit: false,
            tune: None,
            hardware: false,
            audio: AudioSettings::default(),
            crop: CropMode::Auto,
            subtitles: SubtitleSettings::default(),
            container: Container::Mkv,
            extra_args: vec![],
        }
    }
}

impl EncodeSettings {
    pub fn effective_crf(&self) -> u8 {
        self.crf.unwrap_or_else(|| self.content_type.default_crf(self.codec))
    }
    pub fn effective_preset(&self) -> String {
        if let Some(p) = &self.preset {
            if !p.is_empty() {
                return p.clone();
            }
        }
        match self.codec {
            Codec::X264 => "veryslow".into(),
            Codec::Hevc => "slow".into(),
            Codec::Av1 => "4".into(),
        }
    }
}

// ---------- media info ----------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VideoStream {
    pub index: usize,
    pub codec: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// Sample aspect ratio (1.0 for square pixels).
    pub sar: f64,
    pub pix_fmt: String,
    pub bit_depth: u8,
    pub bitrate: Option<u64>,
    pub hdr: bool,
}

impl VideoStream {
    /// Display aspect ratio of a w×h region of this stream.
    pub fn dar_of(&self, w: u32, h: u32) -> f64 {
        (w as f64 / h.max(1) as f64) * if self.sar > 0.0 { self.sar } else { 1.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AudioStream {
    pub index: usize,
    pub rel_index: usize,
    pub codec: String,
    pub channels: u32,
    pub channel_layout: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub bitrate: Option<u64>,
    pub is_default: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SubtitleStream {
    pub index: usize,
    pub rel_index: usize,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub forced: bool,
    pub is_default: bool,
    pub text_based: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MediaInfo {
    pub path: String,
    pub size_bytes: u64,
    pub duration_secs: f64,
    pub container: String,
    pub bitrate: Option<u64>,
    pub video: Option<VideoStream>,
    pub audio: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
    pub chapters: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Crop {
    pub w: u32,
    pub h: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SubCandidate {
    pub path: String,
    pub language: Option<String>,
    /// "same-name" | "subs-folder" | "downloaded"
    pub source: String,
    pub score: u32,
}

// ---------- jobs ----------

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Probing,
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
    Interrupted,
    Skipped,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Probing => "probing",
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Interrupted => "interrupted",
            JobStatus::Skipped => "skipped",
        }
    }
    pub fn is_finished(self) -> bool {
        matches!(self, JobStatus::Done | JobStatus::Failed | JobStatus::Cancelled | JobStatus::Skipped)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Progress {
    pub percent: f64,
    pub frame: u64,
    pub fps: f64,
    pub speed: f64,
    pub out_time_secs: f64,
    pub out_size_bytes: u64,
    pub eta_secs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Job {
    pub id: String,
    pub source: String,
    /// Folder the source was added from (used to mirror structure).
    pub root: String,
    pub output: String,
    pub settings: EncodeSettings,
    pub status: JobStatus,
    /// Added while the queue was running: waits for the user to review settings and press Start.
    pub held: bool,
    pub info: Option<MediaInfo>,
    pub crop: Option<Crop>,
    pub sub_candidates: Vec<SubCandidate>,
    /// Subtitle file picked automatically (mode Auto) after probing.
    pub auto_subtitle: Option<String>,
    pub estimate: Option<Estimate>,
    pub progress: Progress,
    pub error: Option<String>,
    pub log_tail: String,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub in_size: u64,
    pub out_size: Option<u64>,
    pub elapsed_secs: Option<f64>,
    pub avg_fps: Option<f64>,
    pub avg_speed: Option<f64>,
    pub pid: Option<u32>,
    pub order: i64,
}

impl Default for Job {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: String::new(),
            root: String::new(),
            output: String::new(),
            settings: EncodeSettings::default(),
            status: JobStatus::Pending,
            held: false,
            info: None,
            crop: None,
            sub_candidates: vec![],
            auto_subtitle: None,
            estimate: None,
            progress: Progress::default(),
            error: None,
            log_tail: String::new(),
            created_at: 0,
            started_at: None,
            finished_at: None,
            in_size: 0,
            out_size: None,
            elapsed_secs: None,
            avg_fps: None,
            avg_speed: None,
            pid: None,
            order: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Estimate {
    pub size_bytes: u64,
    pub seconds: f64,
    pub out_width: u32,
    pub out_height: u32,
    pub crf: u8,
    pub video_kbps: u32,
    pub audio_kbps: u32,
    pub fps_assumed: f64,
    pub calibrated: bool,
    pub note: Option<String>,
}

// ---------- settings ----------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NamingSettings {
    pub add_tag: bool,
    pub tag_template: String,
    pub add_signature: bool,
    pub signature: String,
}

impl Default for NamingSettings {
    fn default() -> Self {
        Self {
            add_tag: false,
            tag_template: "[{res} {codec}]".into(),
            add_signature: false,
            signature: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct OpenSubtitlesSettings {
    pub api_key: String,
    pub username: String,
    pub password: String,
    pub languages: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    pub output_dir: Option<String>,
    pub preserve_structure: bool,
    pub recursive: bool,
    pub parallel_jobs: u32,
    pub prevent_sleep: bool,
    /// "ask" | "always" | "never"
    pub auto_resume: String,
    pub skip_existing: bool,
    /// Start files added while the queue is running without waiting for review.
    pub auto_start_new: bool,
    pub notify_on_finish: bool,
    /// "none" | "notify" | "sleep" | "shutdown"
    pub post_queue_action: String,
    pub ffmpeg_path: Option<String>,
    pub naming: NamingSettings,
    pub defaults: EncodeSettings,
    pub opensubtitles: OpenSubtitlesSettings,
    pub save_subs_next_to_video: bool,
    pub advanced_mode: bool,
    /// "system" | "light" | "dark"
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_dir: None,
            preserve_structure: true,
            recursive: true,
            parallel_jobs: 0, // 0 = use machine suggestion
            prevent_sleep: true,
            auto_resume: "ask".into(),
            skip_existing: true,
            auto_start_new: false,
            notify_on_finish: true,
            post_queue_action: "notify".into(),
            ffmpeg_path: None,
            naming: NamingSettings::default(),
            defaults: EncodeSettings::default(),
            opensubtitles: OpenSubtitlesSettings::default(),
            save_subs_next_to_video: true,
            advanced_mode: false,
            theme: "system".into(),
        }
    }
}

// ---------- runtime info ----------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Capabilities {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    /// "bundled" | "custom" | "path"
    pub source: String,
    pub version: String,
    pub encoders: Vec<String>,
    pub has_aac_at: bool,
    pub has_fdk_aac: bool,
    pub has_x264: bool,
    pub has_x265: bool,
    pub has_svtav1: bool,
    pub hw_h264: Option<String>,
    pub hw_hevc: Option<String>,
}

impl Capabilities {
    pub fn audio_encoder(&self) -> &'static str {
        if self.has_aac_at {
            "aac_at"
        } else if self.has_fdk_aac {
            "libfdk_aac"
        } else {
            "aac"
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MachineInfo {
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub total_mem_gb: f64,
    pub cpu_brand: String,
    pub os: String,
    pub suggested_jobs: u32,
    pub suggested_jobs_hd: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct QueueStats {
    pub total: usize,
    pub done: usize,
    pub running: usize,
    pub pending: usize,
    pub failed: usize,
    pub speed: f64,
    pub fps: f64,
    pub eta_secs: Option<f64>,
    pub in_bytes_done: u64,
    pub out_bytes_done: u64,
    pub cpu_percent: f32,
    pub paused: bool,
    pub parallel_jobs: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HistoryRow {
    pub id: i64,
    pub source: String,
    pub output: String,
    pub codec: String,
    pub resolution: u32,
    pub crf: u8,
    pub content_type: String,
    pub in_size: u64,
    pub out_size: u64,
    pub duration_secs: f64,
    pub elapsed_secs: f64,
    pub avg_fps: f64,
    pub avg_speed: f64,
    pub finished_at: i64,
    pub parallel_jobs: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CalibrationRow {
    pub key: String,
    pub throughput_fps: f64,
    pub samples: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestEncodeResult {
    pub clip_secs: f64,
    pub start_secs: f64,
    pub out_path: String,
    pub clip_size_bytes: u64,
    pub extrapolated_size_bytes: u64,
    pub elapsed_secs: f64,
    pub fps: f64,
    pub speed: f64,
    pub extrapolated_secs: f64,
    pub before_jpeg_b64: Option<String>,
    pub after_jpeg_b64: Option<String>,
    pub out_width: u32,
    pub out_height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkPoint {
    pub jobs: u32,
    pub total_fps: f64,
    pub per_job_fps: f64,
}
