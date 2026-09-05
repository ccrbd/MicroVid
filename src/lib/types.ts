// Mirrors src-tauri/src/models.rs (serde JSON shapes).
export type Codec = "x264" | "hevc" | "av1";
export type Resolution = "360" | "480" | "576" | "720" | "1080" | "source";
export type ContentType = "general" | "drama" | "sitcom" | "animation" | "action" | "news";
export type Channels = "stereo" | "mono";
export type SubtitleMode = "auto" | "file" | "source" | "none";
export type Container = "mkv" | "mp4";
export type JobStatus = "probing" | "pending" | "running" | "done" | "failed" | "cancelled" | "interrupted" | "skipped";

export type CropMode = { mode: "auto" } | { mode: "none" } | { mode: "manual"; w: number; h: number; x: number; y: number };

export type AudioMode = "default" | "all" | "select";
export interface AudioSettings {
  bitrate_kbps: number;
  channels: Channels;
  mode: AudioMode;
  tracks: number[];
  default_track: number | null;
}

export interface SubtitleSettings {
  mode: SubtitleMode;
  file: string | null;
  delay_ms: number;
  burn_in: boolean;
  keep_source_subs: boolean;
  source_tracks: number[] | null;
  /** -1 = external file, N = source stream s:N, null = auto */
  default_track: number | null;
  language: string;
}

export interface EncodeSettings {
  codec: Codec;
  resolution: Resolution;
  content_type: ContentType;
  crf: number | null;
  preset: string | null;
  ten_bit: boolean;
  tune: string | null;
  hardware: boolean;
  audio: AudioSettings;
  crop: CropMode;
  subtitles: SubtitleSettings;
  container: Container;
  extra_args: string[];
}

export const defaultEncodeSettings = (): EncodeSettings => ({
  codec: "hevc",
  resolution: "480",
  content_type: "general",
  crf: null,
  preset: null,
  ten_bit: false,
  tune: null,
  hardware: false,
  audio: { bitrate_kbps: 80, channels: "stereo", mode: "default", tracks: [], default_track: null },
  crop: { mode: "auto" },
  subtitles: { mode: "auto", file: null, delay_ms: 0, burn_in: false, keep_source_subs: true, source_tracks: null, default_track: null, language: "eng" },
  container: "mkv",
  extra_args: [],
});

export interface VideoStream {
  index: number;
  codec: string;
  profile: string | null;
  width: number;
  height: number;
  fps: number;
  sar: number;
  pix_fmt: string;
  bit_depth: number;
  bitrate: number | null;
  hdr: boolean;
}
export interface AudioStream {
  index: number;
  rel_index: number;
  codec: string;
  channels: number;
  channel_layout: string | null;
  language: string | null;
  title: string | null;
  bitrate: number | null;
  is_default: boolean;
}
export interface SubtitleStream {
  index: number;
  rel_index: number;
  codec: string;
  language: string | null;
  title: string | null;
  forced: boolean;
  is_default: boolean;
  text_based: boolean;
}
export interface MediaInfo {
  path: string;
  size_bytes: number;
  duration_secs: number;
  container: string;
  bitrate: number | null;
  video: VideoStream | null;
  audio: AudioStream[];
  subtitles: SubtitleStream[];
  chapters: number;
}
export interface Crop { w: number; h: number; x: number; y: number }
export interface SubCandidate { path: string; language: string | null; source: string; score: number }
export interface Progress {
  percent: number;
  frame: number;
  fps: number;
  speed: number;
  out_time_secs: number;
  out_size_bytes: number;
  eta_secs: number | null;
}
export interface Estimate {
  size_bytes: number;
  seconds: number;
  out_width: number;
  out_height: number;
  crf: number;
  video_kbps: number;
  audio_kbps: number;
  fps_assumed: number;
  calibrated: boolean;
  note: string | null;
}
export interface Job {
  id: string;
  source: string;
  root: string;
  output: string;
  settings: EncodeSettings;
  status: JobStatus;
  held: boolean;
  info: MediaInfo | null;
  crop: Crop | null;
  sub_candidates: SubCandidate[];
  auto_subtitle: string | null;
  estimate: Estimate | null;
  progress: Progress;
  error: string | null;
  log_tail: string;
  created_at: number;
  started_at: number | null;
  finished_at: number | null;
  in_size: number;
  out_size: number | null;
  elapsed_secs: number | null;
  avg_fps: number | null;
  avg_speed: number | null;
  pid: number | null;
  order: number;
}

export interface NamingSettings { add_tag: boolean; tag_template: string; add_signature: boolean; signature: string }
export interface OpenSubtitlesSettings { api_key: string; username: string; password: string; languages: string }
export interface AppSettings {
  output_dir: string | null;
  preserve_structure: boolean;
  recursive: boolean;
  parallel_jobs: number;
  prevent_sleep: boolean;
  auto_resume: "ask" | "always" | "never";
  skip_existing: boolean;
  auto_start_new: boolean;
  notify_on_finish: boolean;
  post_queue_action: "none" | "notify" | "sleep" | "shutdown";
  ffmpeg_path: string | null;
  naming: NamingSettings;
  defaults: EncodeSettings;
  opensubtitles: OpenSubtitlesSettings;
  save_subs_next_to_video: boolean;
  advanced_mode: boolean;
  theme: "system" | "light" | "dark";
}
export interface Capabilities {
  ffmpeg_path: string;
  ffprobe_path: string;
  source: string;
  version: string;
  encoders: string[];
  has_aac_at: boolean;
  has_fdk_aac: boolean;
  has_x264: boolean;
  has_x265: boolean;
  has_svtav1: boolean;
  hw_h264: string | null;
  hw_hevc: string | null;
}
export interface MachineInfo {
  physical_cores: number;
  logical_cores: number;
  total_mem_gb: number;
  cpu_brand: string;
  os: string;
  suggested_jobs: number;
  suggested_jobs_hd: number;
}
export interface QueueStats {
  total: number;
  done: number;
  running: number;
  pending: number;
  failed: number;
  speed: number;
  fps: number;
  eta_secs: number | null;
  in_bytes_done: number;
  out_bytes_done: number;
  cpu_percent: number;
  paused: boolean;
  parallel_jobs: number;
}
export interface HistoryRow {
  id: number;
  source: string;
  output: string;
  codec: string;
  resolution: number;
  crf: number;
  content_type: string;
  in_size: number;
  out_size: number;
  duration_secs: number;
  elapsed_secs: number;
  avg_fps: number;
  avg_speed: number;
  finished_at: number;
  parallel_jobs: number;
}
export interface CalibrationRow { key: string; throughput_fps: number; samples: number }
export interface TestEncodeResult {
  clip_secs: number;
  start_secs: number;
  out_path: string;
  clip_size_bytes: number;
  extrapolated_size_bytes: number;
  elapsed_secs: number;
  fps: number;
  speed: number;
  extrapolated_secs: number;
  before_jpeg_b64: string | null;
  after_jpeg_b64: string | null;
  out_width: number;
  out_height: number;
}
export interface BenchmarkPoint { jobs: number; total_fps: number; per_job_fps: number }
export interface OsSearchResult {
  file_id: number;
  file_name: string;
  language: string;
  release: string;
  download_count: number;
  fps: number | null;
  hearing_impaired: boolean;
  from_trusted: boolean;
  moviehash_match: boolean;
  title: string;
}
export interface StartupInfo {
  interrupted: number;
  machine: MachineInfo;
  settings: AppSettings;
  capabilities: Capabilities | null;
  capabilities_error: string | null;
  default_output_dir: string;
  jobs: Job[];
  paused: boolean;
}
export interface Analytics { history: HistoryRow[]; calibration: CalibrationRow[]; session_start: number }
