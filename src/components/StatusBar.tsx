import { Cpu } from "lucide-react";
import { useStore } from "../lib/store";
import { bytes, duration, fps } from "../lib/format";

export default function StatusBar() {
  const stats = useStore((s) => s.stats);
  const setView = useStore((s) => s.setView);
  const view = useStore((s) => s.view);
  const sep = <span style={{ color: "var(--mv-faint)" }}>|</span>;
  const active = stats && stats.running > 0;
  const saved = stats ? stats.in_bytes_done - stats.out_bytes_done : 0;

  return (
    <button
      className="mono flex w-full items-center gap-3 border-t px-4 py-1.5 text-left text-[12px]"
      style={{ background: "var(--mv-panel)", borderColor: "var(--mv-border)", color: "var(--mv-muted)", cursor: "pointer" }}
      onClick={() => setView(view === "analytics" ? "queue" : "analytics")}
      title="Open analytics"
    >
      {!stats || stats.total === 0 ? (
        <span>Idle · drop files or folders to start</span>
      ) : active ? (
        <>
          <span><b style={{ color: "var(--mv-text)", fontWeight: 500 }}>Converting:</b> {stats.done + stats.running}/{stats.total} files</span>
          {sep}
          <span><b style={{ color: "var(--mv-text)", fontWeight: 500 }}>Speed:</b> {stats.speed.toFixed(1)}x ({fps(stats.fps)} fps)</span>
          {sep}
          <span><b style={{ color: "var(--mv-text)", fontWeight: 500 }}>ETA:</b> {duration(stats.eta_secs, true)}</span>
          {saved > 0 && (<>{sep}<span><b style={{ color: "var(--mv-text)", fontWeight: 500 }}>Saved:</b> {bytes(saved)}</span></>)}
        </>
      ) : (
        <>
          <span><b style={{ color: "var(--mv-text)", fontWeight: 500 }}>{stats.paused ? "Paused" : "Idle"}:</b> {stats.done}/{stats.total} done{stats.pending ? `, ${stats.pending} waiting` : ""}{stats.failed ? `, ${stats.failed} failed` : ""}</span>
          {stats.pending > 0 && stats.eta_secs != null && (<>{sep}<span>Est. remaining {duration(stats.eta_secs, true)}</span></>)}
          {saved > 0 && (<>{sep}<span>Saved {bytes(saved)}</span></>)}
        </>
      )}
      <span className="flex-1" />
      {stats && (
        <span className="flex items-center gap-1.5">
          <Cpu size={13} /> {stats.running}/{stats.parallel_jobs} jobs · {Math.round(stats.cpu_percent)}% CPU
        </span>
      )}
    </button>
  );
}
