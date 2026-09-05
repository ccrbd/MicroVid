import { useState } from "react";
import { Loader2 } from "lucide-react";
import Modal from "./Modal";
import { ipc } from "../lib/ipc";
import type { Job, TestEncodeResult } from "../lib/types";
import { bytes, duration, fps } from "../lib/format";

export default function TestEncodeModal({ job, onClose }: { job: Job; onClose: () => void }) {
  const dur = job.info?.duration_secs ?? 0;
  const clip = Math.min(30, dur);
  const [start, setStart] = useState(Math.max(0, dur / 2 - clip / 2));
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<TestEncodeResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wipe, setWipe] = useState(50);

  const run = async () => {
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      setResult(await ipc.testEncode(job.id, start));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Modal title="Test encode · 30 seconds" onClose={onClose} width={860}>
      <div className="mb-3 text-[12px]" style={{ color: "var(--mv-muted)" }}>
        Encodes a 30 second clip with the current settings so you can check real size, speed and quality before committing the whole file. Starts from the middle by default so you skip intros and credits.
      </div>
      <div className="mb-3 flex items-center gap-3">
        <span className="text-[12px]" style={{ color: "var(--mv-muted)" }}>Start at</span>
        <input type="range" className="flex-1" min={0} max={Math.max(0, dur - clip)} step={1} value={start} disabled={running} onChange={(e) => setStart(Number(e.target.value))} />
        <span className="mono w-16 text-right text-[12px]">{duration(start)}</span>
        <button className="mv-btn primary" onClick={run} disabled={running || !job.info}>
          {running ? <Loader2 size={14} className="spin" /> : null} {running ? "Encoding…" : result ? "Run again" : "Run test"}
        </button>
      </div>
      {error && <div className="mb-3 text-[12px]" style={{ color: "var(--mv-danger)" }}>{error}</div>}
      {result && (
        <>
          <div className="mb-3 grid grid-cols-4 gap-2">
            {[
              ["Full file size", `≈ ${bytes(result.extrapolated_size_bytes)}`, `${job.in_size ? Math.round((1 - result.extrapolated_size_bytes / job.in_size) * 100) : 0}% smaller`],
              ["Full file time", `≈ ${duration(result.extrapolated_secs, true)}`, `${result.speed.toFixed(2)}x realtime`],
              ["Speed", `${fps(result.fps)} fps`, `clip took ${duration(result.elapsed_secs, true)}`],
              ["Output", `${result.out_width}×${result.out_height}`, `${bytes(result.clip_size_bytes)} for ${result.clip_secs.toFixed(0)} s`],
            ].map(([l, v, s]) => (
              <div key={l} className="rounded-lg p-2.5" style={{ background: "var(--mv-panel-2)" }}>
                <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{l}</div>
                <div className="text-[15px] font-medium">{v}</div>
                <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{s}</div>
              </div>
            ))}
          </div>
          {result.before_jpeg_b64 && result.after_jpeg_b64 && (
            <div>
              <div className="mb-1 flex items-center justify-between text-[11px]" style={{ color: "var(--mv-muted)" }}>
                <span>Source (scaled to output size)</span>
                <span>Drag the slider to compare</span>
                <span>Encoded</span>
              </div>
              <div className="relative w-full overflow-hidden rounded-lg" style={{ aspectRatio: `${result.out_width} / ${result.out_height}`, background: "#000" }}>
                <img src={`data:image/jpeg;base64,${result.before_jpeg_b64}`} className="absolute inset-0 h-full w-full object-contain" alt="source frame" draggable={false} />
                <img src={`data:image/jpeg;base64,${result.after_jpeg_b64}`} className="absolute inset-0 h-full w-full object-contain" alt="encoded frame" draggable={false} style={{ clipPath: `inset(0 0 0 ${wipe}%)` }} />
                <div className="absolute top-0 bottom-0 w-px" style={{ left: `${wipe}%`, background: "var(--mv-accent)" }} />
              </div>
              <input type="range" className="mt-2 w-full" min={0} max={100} step={1} value={wipe} onChange={(e) => setWipe(Number(e.target.value))} />
              <div className="mt-1 flex gap-2">
                <button className="mv-btn" onClick={() => ipc.openPath(result.out_path)}>Open clip in player</button>
                <button className="mv-btn" onClick={() => ipc.revealPath(result.out_path)}>Show clip file</button>
              </div>
            </div>
          )}
        </>
      )}
    </Modal>
  );
}
