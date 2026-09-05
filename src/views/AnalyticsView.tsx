import { useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { Download, FolderOpen, RefreshCw } from "lucide-react";
import { ipc } from "../lib/ipc";
import { useStore } from "../lib/store";
import type { Analytics, HistoryRow } from "../lib/types";
import { bytes, duration, fileName, fps } from "../lib/format";

function Metric({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rounded-lg p-3" style={{ background: "var(--mv-panel-2)" }}>
      <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{label}</div>
      <div className="text-[18px] font-medium">{value}</div>
      {sub && <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{sub}</div>}
    </div>
  );
}

function Bars({ rows, title }: { rows: HistoryRow[]; title: string }) {
  const data = rows.slice(0, 30).reverse();
  const max = Math.max(1, ...data.map((r) => r.in_size));
  const w = 640, h = 140, bw = Math.max(4, Math.floor(w / Math.max(1, data.length)) - 3);
  return (
    <div className="mv-card p-3">
      <div className="mb-2 text-[12px] font-medium">{title}</div>
      {data.length === 0 ? (
        <div className="text-[12px]" style={{ color: "var(--mv-faint)" }}>No finished encodes yet.</div>
      ) : (
        <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ height: h }}>
          {data.map((r, i) => {
            const x = i * (bw + 3);
            const hi = (r.in_size / max) * (h - 20);
            const ho = (r.out_size / max) * (h - 20);
            return (
              <g key={r.id}>
                <title>{`${fileName(r.source)}: ${bytes(r.in_size)} → ${bytes(r.out_size)}`}</title>
                <rect x={x} y={h - 16 - hi} width={bw} height={hi} fill="var(--mv-border)" rx={1} />
                <rect x={x} y={h - 16 - ho} width={bw} height={ho} fill="var(--mv-accent)" rx={1} />
              </g>
            );
          })}
          <text x={0} y={h - 3} fontSize={10} fill="var(--mv-faint)">grey = source · green = output · last {data.length} files</text>
        </svg>
      )}
    </div>
  );
}

function SpeedLine({ rows }: { rows: HistoryRow[] }) {
  const data = rows.slice(0, 40).reverse();
  const w = 640, h = 140;
  const max = Math.max(1, ...data.map((r) => r.avg_fps));
  const pts = data.map((r, i) => `${(i / Math.max(1, data.length - 1)) * (w - 10) + 5},${h - 16 - (r.avg_fps / max) * (h - 26)}`).join(" ");
  return (
    <div className="mv-card p-3">
      <div className="mb-2 text-[12px] font-medium">Encode speed over time (fps per job)</div>
      {data.length < 2 ? (
        <div className="text-[12px]" style={{ color: "var(--mv-faint)" }}>Needs at least two finished encodes.</div>
      ) : (
        <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ height: h }}>
          <polyline points={pts} fill="none" stroke="var(--mv-accent)" strokeWidth={2} />
          {data.map((r, i) => (
            <circle key={r.id} cx={(i / Math.max(1, data.length - 1)) * (w - 10) + 5} cy={h - 16 - (r.avg_fps / max) * (h - 26)} r={3} fill="var(--mv-accent)">
              <title>{`${fileName(r.source)}: ${fps(r.avg_fps)} fps · ${r.codec} ${r.resolution}p`}</title>
            </circle>
          ))}
          <text x={0} y={h - 3} fontSize={10} fill="var(--mv-faint)">peak {fps(max)} fps</text>
        </svg>
      )}
    </div>
  );
}

export default function AnalyticsView() {
  const [data, setData] = useState<Analytics | null>(null);
  const [scope, setScope] = useState<"all" | "session">("all");
  const machine = useStore((s) => s.machine);
  const stats = useStore((s) => s.stats);
  const doneCount = useStore((s) => s.jobs.filter((j) => j.status === "done").length);
  const showToast = useStore((s) => s.showToast);
  const load = () => ipc.getAnalytics().then(setData);
  useEffect(() => { load(); }, [doneCount]);

  const rows = useMemo(() => {
    if (!data) return [];
    return scope === "session" ? data.history.filter((r) => r.finished_at >= data.session_start) : data.history;
  }, [data, scope]);
  const inB = rows.reduce((a, r) => a + r.in_size, 0);
  const outB = rows.reduce((a, r) => a + r.out_size, 0);
  const secs = rows.reduce((a, r) => a + r.elapsed_secs, 0);
  const media = rows.reduce((a, r) => a + r.duration_secs, 0);
  const byCodec = useMemo(() => {
    const m = new Map<string, { n: number; inB: number; outB: number }>();
    for (const r of rows) {
      const k = `${r.codec} ${r.resolution}p`;
      const e = m.get(k) ?? { n: 0, inB: 0, outB: 0 };
      e.n++; e.inB += r.in_size; e.outB += r.out_size;
      m.set(k, e);
    }
    return [...m.entries()].sort((a, b) => b[1].n - a[1].n);
  }, [rows]);

  const exportCsv = async () => {
    const p = await save({ defaultPath: "microvid-history.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (p) ipc.exportHistoryCsv(p).then((n) => showToast(`Exported ${n} rows`, "success"));
  };

  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="mx-auto max-w-4xl">
        <div className="mb-3 flex items-center gap-2">
          <h1 className="text-[16px] font-medium">Analytics</h1>
          <span className="flex-1" />
          <select className="mv-select" style={{ width: 150 }} value={scope} onChange={(e) => setScope(e.target.value as "all" | "session")}>
            <option value="all">All time</option>
            <option value="session">This session</option>
          </select>
          <button className="mv-btn" onClick={load}><RefreshCw size={13} /></button>
          <button className="mv-btn" onClick={exportCsv}><Download size={13} /> Export CSV</button>
        </div>

        <div className="mb-3 grid grid-cols-5 gap-2">
          <Metric label="Files converted" value={String(rows.length)} sub={stats ? `${stats.running} running now` : undefined} />
          <Metric label="Size before → after" value={`${bytes(inB)} → ${bytes(outB)}`} />
          <Metric label="Space saved" value={inB ? `${Math.round((1 - outB / inB) * 100)}%` : "–"} sub={bytes(inB - outB)} />
          <Metric label="Encode time" value={duration(secs, true)} sub={media ? `${(media / Math.max(1, secs)).toFixed(1)}x realtime avg` : undefined} />
          <Metric label="Video processed" value={duration(media, true)} sub={rows.length ? `${fps(rows.reduce((a, r) => a + r.avg_fps, 0) / rows.length)} fps avg` : undefined} />
        </div>

        <div className="mb-3 grid grid-cols-2 gap-3">
          <Bars rows={rows} title="Size per file: source vs output" />
          <SpeedLine rows={rows} />
        </div>

        <div className="mb-3 grid grid-cols-2 gap-3">
          <div className="mv-card p-3">
            <div className="mb-2 text-[12px] font-medium">By codec and resolution</div>
            {byCodec.length === 0 ? (
              <div className="text-[12px]" style={{ color: "var(--mv-faint)" }}>Nothing yet.</div>
            ) : (
              <table className="w-full text-[12px]">
                <tbody>
                  {byCodec.map(([k, e]) => (
                    <tr key={k} className="border-t" style={{ borderColor: "var(--mv-border)" }}>
                      <td className="py-1">{k}</td>
                      <td className="py-1 text-right">{e.n} files</td>
                      <td className="py-1 text-right">{bytes(e.inB)} → {bytes(e.outB)}</td>
                      <td className="py-1 text-right" style={{ color: "var(--mv-accent-text)" }}>{Math.round((1 - e.outB / Math.max(1, e.inB)) * 100)}%</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
          <div className="mv-card p-3">
            <div className="mb-2 text-[12px] font-medium">This machine</div>
            <div className="text-[12px]" style={{ color: "var(--mv-muted)" }}>
              {machine ? (
                <>
                  <div>{machine.cpu_brand || "CPU"} · {machine.physical_cores} cores ({machine.logical_cores} threads) · {machine.total_mem_gb} GB · {machine.os}</div>
                  <div>Suggested parallel jobs: {machine.suggested_jobs} for ≤480p, {machine.suggested_jobs_hd} for 720p+</div>
                </>
              ) : null}
              <div className="mt-2 text-[11.5px]" style={{ color: "var(--mv-faint)" }}>Calibrated throughput (frames/s across all jobs):</div>
              {data?.calibration.length ? (
                <table className="mono mt-1 w-full text-[11px]">
                  <tbody>
                    {data.calibration.map((c) => (
                      <tr key={c.key}>
                        <td>{c.key}</td>
                        <td className="text-right">{fps(c.throughput_fps)} fps</td>
                        <td className="text-right" style={{ color: "var(--mv-faint)" }}>{c.samples} sample{c.samples === 1 ? "" : "s"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <div className="text-[11.5px]" style={{ color: "var(--mv-faint)" }}>None yet. Estimates use the built-in table until a job or test encode finishes.</div>
              )}
            </div>
          </div>
        </div>

        <div className="mv-card p-3">
          <div className="mb-2 text-[12px] font-medium">History</div>
          {rows.length === 0 ? (
            <div className="text-[12px]" style={{ color: "var(--mv-faint)" }}>Finished encodes will appear here.</div>
          ) : (
            <div className="max-h-[420px] overflow-auto">
              <table className="w-full text-[12px]">
                <thead>
                  <tr style={{ color: "var(--mv-faint)" }}>
                    <th className="py-1 text-left font-normal">File</th>
                    <th className="py-1 text-left font-normal">Encode</th>
                    <th className="py-1 text-right font-normal">Size</th>
                    <th className="py-1 text-right font-normal">Saved</th>
                    <th className="py-1 text-right font-normal">Time</th>
                    <th className="py-1 text-right font-normal">Speed</th>
                    <th className="py-1"></th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r) => (
                    <tr key={r.id} className="border-t" style={{ borderColor: "var(--mv-border)" }}>
                      <td className="max-w-[260px] truncate py-1 pr-2" title={r.source}>{fileName(r.source)}</td>
                      <td className="py-1 pr-2" style={{ color: "var(--mv-muted)" }}>{r.codec} {r.resolution}p crf {r.crf} · {r.content_type}</td>
                      <td className="py-1 text-right whitespace-nowrap">{bytes(r.in_size)} → {bytes(r.out_size)}</td>
                      <td className="py-1 text-right" style={{ color: "var(--mv-accent-text)" }}>{Math.round((1 - r.out_size / Math.max(1, r.in_size)) * 100)}%</td>
                      <td className="py-1 text-right">{duration(r.elapsed_secs, true)}</td>
                      <td className="py-1 text-right">{r.avg_speed.toFixed(1)}x · {fps(r.avg_fps)} fps</td>
                      <td className="py-1 text-right">
                        <button className="mv-btn icon" style={{ height: 22, width: 22 }} title="Show output" onClick={() => ipc.revealPath(r.output)}><FolderOpen size={12} /></button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
