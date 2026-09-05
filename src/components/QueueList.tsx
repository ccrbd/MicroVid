import { AlertTriangle, ArrowUpToLine, Check, Clock, FolderOpen, Loader2, Play, RotateCcw, Trash2, X, XCircle, SkipForward } from "lucide-react";
import { ipc } from "../lib/ipc";
import { useStore } from "../lib/store";
import type { Job } from "../lib/types";
import { bytes, duration, fileName } from "../lib/format";
import { addFiles, addFolder } from "./Header";

function StatusChip({ job }: { job: Job }) {
  const map: Record<Job["status"], [string, string, string]> = {
    probing: ["Analysing", "var(--mv-panel-2)", "var(--mv-muted)"],
    pending: ["Pending", "var(--mv-panel-2)", "var(--mv-muted)"],
    running: ["Encoding", "var(--mv-info-soft)", "var(--mv-info-text)"],
    done: ["Done", "var(--mv-accent-soft)", "var(--mv-accent-text)"],
    failed: ["Failed", "var(--mv-danger-soft)", "var(--mv-danger)"],
    cancelled: ["Cancelled", "var(--mv-panel-2)", "var(--mv-muted)"],
    interrupted: ["Interrupted", "var(--mv-warn-soft)", "var(--mv-warn-text)"],
    skipped: ["Skipped", "var(--mv-warn-soft)", "var(--mv-warn-text)"],
  };
  const [label, bg, fg] = job.held && (job.status === "pending" || job.status === "probing") ? ["Review", "var(--mv-warn-soft)", "var(--mv-warn-text)"] : map[job.status];
  return <span className="mv-chip" style={{ background: bg, color: fg }}>{label}</span>;
}

function Icon({ job }: { job: Job }) {
  switch (job.status) {
    case "done": return <Check size={15} style={{ color: "var(--mv-accent)" }} />;
    case "running": return <Loader2 size={15} className="spin" style={{ color: "var(--mv-info-text)" }} />;
    case "probing": return <Loader2 size={15} className="spin" style={{ color: "var(--mv-faint)" }} />;
    case "failed": return <XCircle size={15} style={{ color: "var(--mv-danger)" }} />;
    case "interrupted": return <AlertTriangle size={15} style={{ color: "var(--mv-warn-text)" }} />;
    case "skipped": return <SkipForward size={15} style={{ color: "var(--mv-warn-text)" }} />;
    case "cancelled": return <X size={15} style={{ color: "var(--mv-faint)" }} />;
    default: return <Clock size={15} style={{ color: "var(--mv-faint)" }} />;
  }
}

function meta(job: Job): string {
  const inS = bytes(job.in_size);
  switch (job.status) {
    case "probing": return `${inS} · reading file info…`;
    case "running": {
      const p = job.progress;
      return `${inS} → ≈ ${bytes(job.estimate?.size_bytes)} · ${p.percent.toFixed(0)}% · ${p.speed.toFixed(1)}x · ${duration(p.eta_secs, true)} left`;
    }
    case "done": return `${inS} → ${bytes(job.out_size)} (${job.out_size && job.in_size ? Math.round((1 - job.out_size / job.in_size) * 100) : 0}% smaller) · ${duration(job.elapsed_secs, true)}`;
    case "failed": return job.error ?? "failed";
    case "skipped": return job.error ?? "skipped";
    case "interrupted": return `${inS} · will restart from the beginning`;
    default: {
      if (job.held && job.info) return `${inS} → ≈ ${bytes(job.estimate?.size_bytes)} · waiting for you to review and start`;
      const sub = job.settings.subtitles.mode === "none" ? "no subs" : job.settings.subtitles.mode === "source" ? "source subs" : job.settings.subtitles.mode === "file" && job.settings.subtitles.file ? fileName(job.settings.subtitles.file) : job.auto_subtitle ? fileName(job.auto_subtitle) : job.info?.subtitles.length ? "source subs" : "no subs found";
      return `${inS} → ≈ ${bytes(job.estimate?.size_bytes)} · ≈ ${duration(job.estimate?.seconds, true)} · ${sub}`;
    }
  }
}

export default function QueueList() {
  const jobs = useStore((s) => s.jobs);
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const stats = useStore((s) => s.stats);
  const finished = jobs.filter((j) => ["done", "failed", "cancelled", "skipped"].includes(j.status)).length;
  const failed = jobs.filter((j) => ["failed", "interrupted", "cancelled"].includes(j.status)).map((j) => j.id);
  const held = jobs.filter((j) => j.held && ["pending", "probing"].includes(j.status)).map((j) => j.id);
  const totalIn = jobs.reduce((a, j) => a + j.in_size, 0);
  const totalOut = jobs.reduce((a, j) => a + (j.out_size ?? j.estimate?.size_bytes ?? 0), 0);

  if (jobs.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <div className="flex h-40 w-full max-w-md flex-col items-center justify-center rounded-xl border-2 border-dashed" style={{ borderColor: "var(--mv-border)", color: "var(--mv-muted)" }}>
          <div className="text-[15px] font-medium" style={{ color: "var(--mv-text)" }}>Drop files or folders here</div>
          <div className="mt-1 text-[12px]">A single episode, a season folder, or a whole series with sub-folders</div>
          <div className="mt-4 flex gap-2">
            <button className="mv-btn" onClick={addFiles}>Add files</button>
            <button className="mv-btn" onClick={addFolder}>Add folder</button>
          </div>
        </div>
        <div className="max-w-md text-[12px]" style={{ color: "var(--mv-faint)" }}>
          Defaults: HEVC · 480p · General · AAC 80k · subtitles picked from the same folder · MKV. Change them per file or in Settings.
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b px-3 py-2 text-[12px]" style={{ borderColor: "var(--mv-border)", color: "var(--mv-muted)" }}>
        <span>
          Queue · {jobs.length} file{jobs.length === 1 ? "" : "s"} · {bytes(totalIn)} → ≈ {bytes(totalOut)}
          {stats?.paused === false && stats.running > 0 ? "" : stats?.paused ? " · paused" : ""}
        </span>
        <span className="flex-1" />
        {held.length > 0 && (
          <button className="mv-btn primary" style={{ height: 24 }} onClick={() => ipc.releaseJobs(held)} title="Start the files that are waiting for review">
            <Play size={13} /> Start {held.length} new
          </button>
        )}
        {failed.length > 0 && (
          <button className="mv-btn" style={{ height: 24 }} onClick={() => ipc.retryJobs(failed)} title="Re-queue failed, cancelled and interrupted jobs">
            <RotateCcw size={13} /> Retry {failed.length}
          </button>
        )}
        {finished > 0 && (
          <button className="mv-btn" style={{ height: 24 }} onClick={() => ipc.clearFinished()}>
            Clear finished
          </button>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {jobs.map((job) => {
          const sel = job.id === selectedId;
          const showRel = job.root && job.source.startsWith(job.root) && job.source.slice(job.root.length + 1).includes("/");
          const rel = showRel ? job.source.slice(job.root.length + 1) : fileName(job.source);
          return (
            <div
              key={job.id}
              className="group grid cursor-pointer items-center gap-2 border-b px-3 py-2"
              style={{ gridTemplateColumns: "18px minmax(0,1fr) auto", borderColor: "var(--mv-border)", background: sel ? "var(--mv-panel)" : "transparent" }}
              onClick={() => select(job.id)}
            >
              <Icon job={job} />
              <div className="min-w-0">
                <div className="truncate text-[13px]" title={job.source}>{rel}</div>
                <div className="truncate text-[11px]" style={{ color: job.status === "failed" ? "var(--mv-danger)" : "var(--mv-faint)" }}>{meta(job)}</div>
                {job.status === "running" && (
                  <div className="mt-1 h-[3px] rounded" style={{ background: "var(--mv-border)" }}>
                    <div className="h-full rounded" style={{ width: `${job.progress.percent}%`, background: "var(--mv-accent)" }} />
                  </div>
                )}
              </div>
              <div className="flex items-center gap-1">
                <span className="hidden gap-1 group-hover:flex">
                  {job.status === "done" && (
                    <button className="mv-btn icon" style={{ height: 24, width: 24 }} title="Show in folder" onClick={(e) => { e.stopPropagation(); ipc.revealPath(job.output); }}>
                      <FolderOpen size={13} />
                    </button>
                  )}
                  {job.status === "pending" && jobs.findIndex((j) => j.status === "pending") !== jobs.indexOf(job) && (
                    <button className="mv-btn icon" style={{ height: 24, width: 24 }} title="Encode next" onClick={(e) => { e.stopPropagation(); ipc.reorderJobs([job.id, ...jobs.filter((j) => j.id !== job.id).map((j) => j.id)]); }}>
                      <ArrowUpToLine size={13} />
                    </button>
                  )}
                  {["failed", "cancelled", "interrupted", "skipped"].includes(job.status) && (
                    <button className="mv-btn icon" style={{ height: 24, width: 24 }} title="Retry" onClick={(e) => { e.stopPropagation(); ipc.retryJobs([job.id]); }}>
                      <RotateCcw size={13} />
                    </button>
                  )}
                  {job.status === "running" ? (
                    <button className="mv-btn icon danger" style={{ height: 24, width: 24 }} title="Cancel" onClick={(e) => { e.stopPropagation(); ipc.cancelJobs([job.id]); }}>
                      <X size={13} />
                    </button>
                  ) : (
                    <button className="mv-btn icon danger" style={{ height: 24, width: 24 }} title="Remove from queue" onClick={(e) => { e.stopPropagation(); ipc.removeJobs([job.id]); }}>
                      <Trash2 size={13} />
                    </button>
                  )}
                </span>
                <StatusChip job={job} />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
