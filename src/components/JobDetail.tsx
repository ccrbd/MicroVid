import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Copy, FileText, FlaskConical, FolderOpen, SlidersHorizontal, Trash2, X } from "lucide-react";
import { ipc } from "../lib/ipc";
import { useStore } from "../lib/store";
import type { EncodeSettings, Job } from "../lib/types";
import { bytes, codecShort, duration, fileName, fps } from "../lib/format";
import SettingsPanel, { defaultCrf } from "./SettingsPanel";
import TestEncodeModal from "./TestEncodeModal";
import OpenSubtitlesModal from "./OpenSubtitlesModal";

const PRESET_LABEL: Record<string, string> = { x264: "x264", hevc: "x265", av1: "SVT-AV1" };

export default function JobDetail() {
  const jobs = useStore((s) => s.jobs);
  const selectedId = useStore((s) => s.selectedId);
  const settings = useStore((s) => s.settings);
  const caps = useStore((s) => s.capabilities);
  const showToast = useStore((s) => s.showToast);
  const job = jobs.find((j) => j.id === selectedId) ?? null;
  const [draft, setDraft] = useState<EncodeSettings | null>(null);
  const [showLog, setShowLog] = useState(false);
  const [modal, setModal] = useState<"test" | "subs" | null>(null);
  const timer = useRef<number | null>(null);
  const draftFor = useRef<string | null>(null);

  useEffect(() => {
    if (!job) {
      setDraft(null);
      draftFor.current = null;
      return;
    }
    if (draftFor.current !== job.id) {
      draftFor.current = job.id;
      setDraft(job.settings);
      setShowLog(false);
    } else if (timer.current == null && JSON.stringify(job.settings) !== JSON.stringify(draft)) {
      // Backend changed the job (e.g. subtitle downloaded) while no local edit is pending.
      setDraft(job.settings);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [job?.id, job?.settings]);

  const update = (next: EncodeSettings) => {
    if (!job) return;
    setDraft(next);
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      ipc.updateJobSettings([job.id], next).catch((e) => showToast(String(e), "error"));
    }, 250);
  };

  const advanced = settings?.advanced_mode ?? false;
  const toggleAdvanced = () => settings && ipc.setSettings({ ...settings, advanced_mode: !advanced }).then((s) => useStore.getState().setSettings(s));
  const pickOutputDir = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string" && settings) ipc.setSettings({ ...settings, output_dir: picked }).then((s) => useStore.getState().setSettings(s));
  };

  if (!job || !draft) {
    return (
      <div className="flex h-full items-center justify-center text-[12.5px]" style={{ color: "var(--mv-faint)" }}>
        Select a file to see its details
      </div>
    );
  }
  const info = job.info;
  const v = info?.video ?? null;
  const est = job.estimate;
  const locked = job.status === "running";
  const pendingIds = jobs.filter((j) => j.id !== job.id && ["pending", "probing", "interrupted", "failed", "cancelled", "skipped"].includes(j.status)).map((j) => j.id);
  const crf = draft.crf ?? defaultCrf(draft);
  const preset = draft.preset ?? { x264: "veryslow", hevc: "slower", av1: "preset 4" }[draft.codec];

  return (
    <div className="flex h-full flex-col overflow-y-auto p-3">
      <div className="mb-1 flex items-center gap-2">
        <span className="truncate text-[14px] font-medium" title={job.source}>{fileName(job.source)}</span>
        <span className="flex-1" />
        <button className="mv-btn" onClick={toggleAdvanced} style={advanced ? { borderColor: "var(--mv-accent)" } : undefined}>
          <SlidersHorizontal size={14} /> {advanced ? "Simple" : "Advanced"}
        </button>
      </div>

      <div className="mb-3 grid grid-cols-[72px_minmax(0,1fr)] gap-x-3 gap-y-1 text-[12px]">
        <span style={{ color: "var(--mv-faint)" }}>Video</span>
        <span>{v ? `${v.width}×${v.height} · ${fps(v.fps)} fps · ${v.codec.toUpperCase()}${v.bit_depth > 8 ? ` ${v.bit_depth}-bit` : ""}${v.hdr ? " HDR" : ""} · ${duration(info?.duration_secs)}` : job.status === "probing" ? "analysing…" : job.error ?? "unknown"}</span>
        <span style={{ color: "var(--mv-faint)" }}>Audio</span>
        <span>{info?.audio.length ? info.audio.map((a) => `${a.codec.toUpperCase()} ${a.channel_layout ?? `${a.channels}ch`}${a.language ? ` ${a.language}` : ""}${a.bitrate ? ` · ${Math.round(a.bitrate / 1000)} kb/s` : ""}`).join(" · ") : info ? "none" : "–"}</span>
        <span style={{ color: "var(--mv-faint)" }}>Crop</span>
        <span>{job.crop ? `auto ${job.crop.w}×${job.crop.h} (${(job.crop.w / job.crop.h).toFixed(2)}:1 after bars)` : info ? "no black bars found" : "–"}</span>
        <span style={{ color: "var(--mv-faint)" }}>Subtitles</span>
        <span style={{ color: job.auto_subtitle || info?.subtitles.length ? "var(--mv-accent-text)" : undefined }}>
          {job.auto_subtitle ? `${fileName(job.auto_subtitle)} (folder)` : ""}
          {job.auto_subtitle && info?.subtitles.length ? " · " : ""}
          {info?.subtitles.length ? `${info.subtitles.length} in file (${info.subtitles.map((s) => s.language ?? s.codec).join(", ")})` : ""}
          {!job.auto_subtitle && !info?.subtitles.length ? (info ? "none found" : "–") : ""}
        </span>
        <span style={{ color: "var(--mv-faint)" }}>Output</span>
        <span className="truncate" title={job.output}>{job.output}</span>
      </div>

      <SettingsPanel value={draft} onChange={update} advanced={advanced} job={job} caps={caps} disabled={locked} onSearchSubs={() => setModal("subs")} />

      <div className="mv-row">
        <label>Output folder</label>
        <div className="flex gap-1.5">
          <input className="mv-input" readOnly value={settings?.output_dir ?? useStore.getState().defaultOutputDir} />
          <button className="mv-btn icon" onClick={pickOutputDir} title="Choose output folder"><FolderOpen size={14} /></button>
        </div>
      </div>

      <div className="my-2 grid grid-cols-3 gap-2">
        <div className="rounded-lg p-2.5" style={{ background: "var(--mv-panel-2)" }}>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>Estimated size</div>
          <div className="text-[16px] font-medium">{est ? `≈ ${bytes(est.size_bytes)}` : "–"}</div>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{est && job.in_size ? `${Math.round((1 - est.size_bytes / job.in_size) * 100)}% smaller · ${est.out_width}×${est.out_height}` : ""}</div>
        </div>
        <div className="rounded-lg p-2.5" style={{ background: "var(--mv-panel-2)" }}>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>Estimated time</div>
          <div className="text-[16px] font-medium">{est ? `≈ ${duration(est.seconds, true)}` : "–"}</div>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{est ? `~${fps(est.fps_assumed)} fps${est.calibrated ? " · calibrated" : " · table estimate"}` : ""}</div>
        </div>
        <div className="rounded-lg p-2.5" style={{ background: "var(--mv-panel-2)" }}>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>Quality</div>
          <div className="text-[16px] font-medium">CRF {crf}</div>
          <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{draft.hardware ? "hardware fast mode" : `${PRESET_LABEL[draft.codec]} ${preset} · ${codecShort[draft.codec]}`}</div>
        </div>
      </div>
      {est?.note && <div className="mb-2 text-[11.5px]" style={{ color: "var(--mv-warn-text)" }}>{est.note}</div>}

      <div className="mt-auto flex flex-wrap gap-1.5 pt-1">
        <button className="mv-btn" disabled={!info} onClick={() => setModal("test")} title="Encode 30 s from the middle to check size and quality">
          <FlaskConical size={14} /> Test encode
        </button>
        <button className="mv-btn" disabled={pendingIds.length === 0} onClick={() => ipc.updateJobSettings(pendingIds, draft).then(() => showToast(`Applied to ${pendingIds.length} other file${pendingIds.length === 1 ? "" : "s"}`, "success"))} title="Use these settings for every other waiting file">
          <Copy size={14} /> Apply to all pending
        </button>
        <span className="flex-1" />
        {job.log_tail && (
          <button className="mv-btn" onClick={() => setShowLog(!showLog)}><FileText size={14} /> Log</button>
        )}
        {job.status === "done" && (
          <button className="mv-btn" onClick={() => ipc.revealPath(job.output)}><FolderOpen size={14} /> Show</button>
        )}
        {locked ? (
          <button className="mv-btn danger" onClick={() => ipc.cancelJobs([job.id])}><X size={14} /> Cancel</button>
        ) : (
          <button className="mv-btn danger" onClick={() => ipc.removeJobs([job.id])}><Trash2 size={14} /> Remove</button>
        )}
      </div>
      {showLog && (
        <pre className="mono select-text mt-2 max-h-48 overflow-auto rounded-lg p-2 text-[11px] whitespace-pre-wrap" style={{ background: "var(--mv-panel-2)", color: "var(--mv-muted)" }}>{job.log_tail}</pre>
      )}
      {modal === "test" && <TestEncodeModal job={job} onClose={() => setModal(null)} />}
      {modal === "subs" && <OpenSubtitlesModal job={job} onClose={() => setModal(null)} />}
    </div>
  );
}

export function jobTitle(job: Job) {
  return fileName(job.source);
}
