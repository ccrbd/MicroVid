import { open } from "@tauri-apps/plugin-dialog";
import type { Capabilities, EncodeSettings, Job } from "../lib/types";
import { codecLabel, contentLabel, fileName, resolutionLabel } from "../lib/format";

const CRF_TABLE: Record<string, Record<string, number>> = {
  x264: { general: 24, drama: 24, sitcom: 25, animation: 27, action: 22, news: 28 },
  hevc: { general: 26, drama: 26, sitcom: 27, animation: 29, action: 24, news: 30 },
  av1: { general: 34, drama: 34, sitcom: 36, animation: 38, action: 32, news: 40 },
};
export const defaultCrf = (s: EncodeSettings) => CRF_TABLE[s.codec][s.content_type];
const PRESETS: Record<string, string[]> = {
  x264: ["veryslow", "slower", "slow", "medium", "fast", "faster", "veryfast"],
  hevc: ["veryslow", "slower", "slow", "medium", "fast", "faster", "veryfast"],
  av1: ["2", "3", "4", "5", "6", "7", "8", "10"],
};
const DEFAULT_PRESET: Record<string, string> = { x264: "veryslow", hevc: "slower", av1: "4" };
const LANGS = [
  ["eng", "English"], ["spa", "Spanish"], ["fre", "French"], ["ger", "German"], ["ita", "Italian"], ["por", "Portuguese"], ["rus", "Russian"],
  ["jpn", "Japanese"], ["kor", "Korean"], ["chi", "Chinese"], ["ara", "Arabic"], ["hin", "Hindi"], ["ben", "Bengali"], ["tur", "Turkish"], ["dut", "Dutch"],
];

interface Props {
  value: EncodeSettings;
  onChange: (next: EncodeSettings) => void;
  advanced: boolean;
  job?: Job | null;
  caps?: Capabilities | null;
  disabled?: boolean;
  /** Called when the user asks to search OpenSubtitles. */
  onSearchSubs?: () => void;
  compact?: boolean;
}

export default function SettingsPanel({ value: v, onChange, advanced, job, caps, disabled, onSearchSubs }: Props) {
  const set = (patch: Partial<EncodeSettings>) => onChange({ ...v, ...patch });
  const setAudio = (patch: Partial<EncodeSettings["audio"]>) => set({ audio: { ...v.audio, ...patch } });
  const setSubs = (patch: Partial<EncodeSettings["subtitles"]>) => set({ subtitles: { ...v.subtitles, ...patch } });
  const info = job?.info ?? null;
  const cands = job?.sub_candidates ?? [];
  const auto = job?.auto_subtitle ?? null;
  const hasSourceSubs = (info?.subtitles.length ?? 0) > 0;
  const hwName = v.codec === "hevc" ? caps?.hw_hevc : v.codec === "x264" ? caps?.hw_h264 : null;

  const subValue =
    v.subtitles.mode === "file" && v.subtitles.file ? `file:${v.subtitles.file}` : v.subtitles.mode;

  const onSubChange = async (val: string) => {
    if (val === "pick") {
      const picked = await open({ multiple: false, directory: false, filters: [{ name: "Subtitles", extensions: ["srt", "ass", "ssa", "vtt", "sub"] }] });
      if (typeof picked === "string") setSubs({ mode: "file", file: picked });
      return;
    }
    if (val === "search") {
      onSearchSubs?.();
      return;
    }
    if (val.startsWith("file:")) {
      setSubs({ mode: "file", file: val.slice(5) });
      return;
    }
    setSubs({ mode: val as EncodeSettings["subtitles"]["mode"], file: null });
  };

  return (
    <div>
      <div className="mv-row">
        <label>Codec</label>
        <select className="mv-select" value={v.codec} disabled={disabled} onChange={(e) => set({ codec: e.target.value as EncodeSettings["codec"], crf: null, preset: null, tune: null })}>
          <option value="hevc">{codecLabel.hevc} · recommended</option>
          <option value="x264">{codecLabel.x264} · plays everywhere</option>
          <option value="av1">{codecLabel.av1} · smallest, newer devices</option>
        </select>
      </div>
      <div className="mv-row">
        <label>Resolution</label>
        <select className="mv-select" value={v.resolution} disabled={disabled} onChange={(e) => set({ resolution: e.target.value as EncodeSettings["resolution"] })}>
          {Object.entries(resolutionLabel).map(([k, l]) => (
            <option key={k} value={k}>
              {l}
              {info?.video && k !== "source" && info.video.height < Number(k) ? ` (source is ${info.video.height}p, no upscale)` : ""}
            </option>
          ))}
        </select>
      </div>
      <div className="mv-row">
        <label>Content type</label>
        <select className="mv-select" value={v.content_type} disabled={disabled} onChange={(e) => set({ content_type: e.target.value as EncodeSettings["content_type"], crf: null })}>
          {Object.entries(contentLabel).map(([k, l]) => (
            <option key={k} value={k}>{l}</option>
          ))}
        </select>
      </div>
      <div className="mv-row">
        <label>Audio</label>
        <select
          className="mv-select"
          value={`${v.audio.bitrate_kbps}-${v.audio.channels}`}
          disabled={disabled}
          onChange={(e) => {
            const [b, c] = e.target.value.split("-");
            setAudio({ bitrate_kbps: Number(b), channels: c as "stereo" | "mono" });
          }}
        >
          <option value="128-stereo">AAC 128k stereo · high</option>
          <option value="96-stereo">AAC 96k stereo</option>
          <option value="80-stereo">AAC 80k stereo · default</option>
          <option value="64-stereo">AAC 64k stereo</option>
          <option value="48-mono">AAC 48k mono · smallest</option>
        </select>
      </div>
      {job && (
        <div className="mv-row">
          <label>Subtitles</label>
          <select className="mv-select" value={subValue} disabled={disabled} onChange={(e) => onSubChange(e.target.value)}>
            <option value="auto">{auto ? `Auto · ${fileName(auto)}` : hasSourceSubs ? "Auto · keep source tracks" : "Auto · none found"}</option>
            {cands.map((c) => (
              <option key={c.path} value={`file:${c.path}`}>
                {fileName(c.path)}{c.language ? ` (${c.language})` : ""}{c.source === "downloaded" ? " · downloaded" : ""}
              </option>
            ))}
            {v.subtitles.mode === "file" && v.subtitles.file && !cands.some((c) => c.path === v.subtitles.file) && (
              <option value={`file:${v.subtitles.file}`}>{fileName(v.subtitles.file)}</option>
            )}
            <option value="pick">Pick a file…</option>
            <option value="search">Search OpenSubtitles…</option>
            <option value="source">Keep source tracks only</option>
            <option value="none">None</option>
          </select>
        </div>
      )}
      {job && v.subtitles.mode !== "none" && v.subtitles.mode !== "source" && (
        <div className="mv-row">
          <label>Subtitle delay</label>
          <div className="flex items-center gap-2">
            <input className="mv-input" style={{ width: 90 }} type="number" step={100} value={v.subtitles.delay_ms} disabled={disabled} onChange={(e) => setSubs({ delay_ms: Number(e.target.value) || 0 })} />
            <span style={{ color: "var(--mv-muted)" }}>ms · negative = earlier</span>
          </div>
        </div>
      )}

      {advanced && (
        <div className="mt-3 border-t pt-3" style={{ borderColor: "var(--mv-border)" }}>
          <div className="mb-2 text-[12px] font-medium" style={{ color: "var(--mv-muted)" }}>Advanced</div>
          <div className="mv-row">
            <label>CRF (quality)</label>
            <div className="flex items-center gap-2">
              <input type="range" className="flex-1" min={v.codec === "av1" ? 20 : 14} max={v.codec === "av1" ? 55 : 40} step={1} value={v.crf ?? defaultCrf(v)} disabled={disabled} onChange={(e) => set({ crf: Number(e.target.value) })} />
              <span className="mono w-7 text-right">{v.crf ?? defaultCrf(v)}</span>
              <button className="mv-btn" style={{ height: 24, padding: "0 6px" }} disabled={disabled || v.crf == null} onClick={() => set({ crf: null })} title="Back to the content-type default">
                auto
              </button>
            </div>
          </div>
          <div className="mv-row">
            <label>Encoder preset</label>
            <select className="mv-select" value={v.preset ?? ""} disabled={disabled} onChange={(e) => set({ preset: e.target.value || null })}>
              <option value="">{DEFAULT_PRESET[v.codec]} (default)</option>
              {PRESETS[v.codec].filter((p) => p !== DEFAULT_PRESET[v.codec]).map((p) => (
                <option key={p} value={p}>{p}</option>
              ))}
            </select>
          </div>
          {v.codec !== "av1" && (
            <div className="mv-row">
              <label>Tune</label>
              <select className="mv-select" value={v.tune ?? ""} disabled={disabled} onChange={(e) => set({ tune: e.target.value || null })}>
                <option value="">Auto by content type</option>
                <option value="none">Off</option>
                <option value="film">film</option>
                <option value="animation">animation</option>
                <option value="grain">grain</option>
              </select>
            </div>
          )}
          <div className="mv-row">
            <label>Bit depth</label>
            <select className="mv-select" value={v.codec === "av1" ? "10" : v.ten_bit ? "10" : "8"} disabled={disabled || v.codec === "av1"} onChange={(e) => set({ ten_bit: e.target.value === "10" })}>
              <option value="8">8-bit (safest)</option>
              <option value="10">10-bit (smoother gradients, HEVC main10)</option>
            </select>
          </div>
          <div className="mv-row">
            <label>Hardware</label>
            <select className="mv-select" value={v.hardware ? "hw" : "sw"} disabled={disabled || !hwName} onChange={(e) => set({ hardware: e.target.value === "hw" })}>
              <option value="sw">Software (best compression)</option>
              <option value="hw">{hwName ? `Fast mode · ${hwName}` : "Fast mode (no hardware encoder found)"}</option>
            </select>
          </div>
          <div className="mv-row">
            <label>Audio tracks</label>
            <select
              className="mv-select"
              value={v.audio.keep_all_tracks ? "all" : v.audio.track == null ? "default" : String(v.audio.track)}
              disabled={disabled}
              onChange={(e) => {
                const val = e.target.value;
                if (val === "all") setAudio({ keep_all_tracks: true, track: null });
                else if (val === "default") setAudio({ keep_all_tracks: false, track: null });
                else setAudio({ keep_all_tracks: false, track: Number(val) });
              }}
            >
              <option value="default">Default track only</option>
              <option value="all">Keep all tracks</option>
              {info?.audio.map((a) => (
                <option key={a.rel_index} value={a.rel_index}>
                  #{a.rel_index + 1} {a.language ?? "und"} · {a.codec} {a.channel_layout ?? `${a.channels}ch`}{a.title ? ` · ${a.title}` : ""}
                </option>
              ))}
            </select>
          </div>
          <div className="mv-row">
            <label>Crop</label>
            <div className="flex items-center gap-2">
              <select className="mv-select" value={v.crop.mode} disabled={disabled} onChange={(e) => {
                const m = e.target.value;
                if (m === "manual") set({ crop: { mode: "manual", w: job?.crop?.w ?? info?.video?.width ?? 0, h: job?.crop?.h ?? info?.video?.height ?? 0, x: job?.crop?.x ?? 0, y: job?.crop?.y ?? 0 } });
                else set({ crop: { mode: m as "auto" | "none" } });
              }}>
                <option value="auto">Auto{job?.crop ? ` · ${job.crop.w}×${job.crop.h}` : job?.info ? " · no bars found" : ""}</option>
                <option value="none">None</option>
                <option value="manual">Manual…</option>
              </select>
            </div>
          </div>
          {v.crop.mode === "manual" && (
            <div className="mv-row">
              <label></label>
              <div className="flex items-center gap-1">
                {(["w", "h", "x", "y"] as const).map((k) => (
                  <label key={k} className="flex items-center gap-1 text-[11px]" style={{ color: "var(--mv-muted)" }}>
                    {k}
                    <input className="mv-input" style={{ width: 62 }} type="number" value={(v.crop as unknown as { [key: string]: number })[k]} disabled={disabled} onChange={(e) => set({ crop: { ...(v.crop as { mode: "manual"; w: number; h: number; x: number; y: number }), [k]: Number(e.target.value) || 0 } })} />
                  </label>
                ))}
              </div>
            </div>
          )}
          <div className="mv-row">
            <label>Subtitle options</label>
            <div className="flex flex-wrap items-center gap-3 text-[12px]">
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={v.subtitles.keep_source_subs} disabled={disabled} onChange={(e) => setSubs({ keep_source_subs: e.target.checked })} /> keep source tracks
              </label>
              <label className="flex items-center gap-1.5">
                <input type="checkbox" checked={v.subtitles.burn_in} disabled={disabled} onChange={(e) => setSubs({ burn_in: e.target.checked })} /> burn in
              </label>
              <select className="mv-select" style={{ width: 120 }} value={v.subtitles.language} disabled={disabled} onChange={(e) => setSubs({ language: e.target.value })}>
                {LANGS.map(([c, n]) => (
                  <option key={c} value={c}>{n}</option>
                ))}
              </select>
            </div>
          </div>
          <div className="mv-row">
            <label>Container</label>
            <select className="mv-select" value={v.container} disabled={disabled} onChange={(e) => set({ container: e.target.value as "mkv" | "mp4" })}>
              <option value="mkv">MKV (recommended, keeps every subtitle type)</option>
              <option value="mp4">MP4 (text subtitles only)</option>
            </select>
          </div>
          <div className="mv-row">
            <label>Extra ffmpeg args</label>
            <input className="mv-input mono" placeholder="-x265-params aq-mode=3" value={v.extra_args.join(" ")} disabled={disabled} onChange={(e) => set({ extra_args: e.target.value.split(/\s+/).filter(Boolean) })} />
          </div>
        </div>
      )}
    </div>
  );
}
