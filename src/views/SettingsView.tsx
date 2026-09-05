import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Loader2, RefreshCw } from "lucide-react";
import { ipc } from "../lib/ipc";
import { useStore } from "../lib/store";
import type { AppSettings, BenchmarkPoint } from "../lib/types";
import SettingsPanel from "../components/SettingsPanel";
import { bytes, fps } from "../lib/format";

function Section({ title, children, hint }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="mv-card mb-3 p-4">
      <div className="mb-3">
        <div className="text-[13px] font-medium">{title}</div>
        {hint && <div className="text-[11.5px]" style={{ color: "var(--mv-faint)" }}>{hint}</div>}
      </div>
      {children}
    </div>
  );
}

const Check = ({ label, checked, onChange, hint }: { label: string; checked: boolean; onChange: (v: boolean) => void; hint?: string }) => (
  <label className="mb-2 flex items-start gap-2 text-[12.5px]">
    <input type="checkbox" className="mt-0.5" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    <span>
      {label}
      {hint && <span className="block text-[11px]" style={{ color: "var(--mv-faint)" }}>{hint}</span>}
    </span>
  </label>
);

export default function SettingsView() {
  const settings = useStore((s) => s.settings);
  const machine = useStore((s) => s.machine);
  const caps = useStore((s) => s.capabilities);
  const capsError = useStore((s) => s.capabilitiesError);
  const defaultOut = useStore((s) => s.defaultOutputDir);
  const jobs = useStore((s) => s.jobs);
  const selectedId = useStore((s) => s.selectedId);
  const showToast = useStore((s) => s.showToast);
  const [draft, setDraft] = useState<AppSettings | null>(settings);
  const [preview, setPreview] = useState("");
  const [bench, setBench] = useState<BenchmarkPoint[] | null>(null);
  const [benchBusy, setBenchBusy] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (settings && !draft) setDraft(settings);
  }, [settings, draft]);
  useEffect(() => {
    if (draft) ipc.previewOutputName(draft.naming, draft.defaults).then(setPreview);
  }, [draft?.naming, draft?.defaults]);

  const save = (next: AppSettings) => {
    setDraft(next);
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      ipc.setSettings(next).then((s) => useStore.getState().setSettings(s)).catch((e) => showToast(String(e), "error"));
    }, 300);
  };
  if (!draft) return null;
  const d = draft;
  const set = (patch: Partial<AppSettings>) => save({ ...d, ...patch });
  const suggested = machine?.suggested_jobs ?? 2;
  const benchJob = jobs.find((j) => j.id === selectedId && j.info) ?? jobs.find((j) => j.info);

  const runBench = async () => {
    if (!benchJob) return;
    setBenchBusy(true);
    try {
      setBench(await ipc.benchmark(benchJob.id, 3));
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBenchBusy(false);
    }
  };
  const redetect = async () => {
    try {
      const c = await ipc.getCapabilities();
      useStore.getState().init({ capabilities: c, capabilitiesError: null });
      showToast(`ffmpeg ${c.version} (${c.source})`, "success");
    } catch (e) {
      useStore.getState().init({ capabilities: null, capabilitiesError: String(e) });
    }
  };

  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="mx-auto max-w-3xl">
        <Section title="Output" hint="Where finished files go.">
          <div className="mv-row">
            <label>Output folder</label>
            <div className="flex gap-1.5">
              <input className="mv-input" readOnly value={d.output_dir ?? defaultOut} />
              <button className="mv-btn icon" title="Choose" onClick={async () => { const p = await open({ directory: true, multiple: false }); if (typeof p === "string") set({ output_dir: p }); }}><FolderOpen size={14} /></button>
              {d.output_dir && <button className="mv-btn" onClick={() => set({ output_dir: null })}>Default</button>}
            </div>
          </div>
          <Check label="Preserve folder structure" hint="Season and series folders are mirrored inside the output folder." checked={d.preserve_structure} onChange={(v) => set({ preserve_structure: v })} />
          <Check label="Include sub-folders when adding a folder" checked={d.recursive} onChange={(v) => set({ recursive: v })} />
          <Check label="Skip files whose output already exists" checked={d.skip_existing} onChange={(v) => set({ skip_existing: v })} />
        </Section>

        <Section title="Naming" hint={`Preview: ${preview}`}>
          <Check label="Add encode tag to the file name" checked={d.naming.add_tag} onChange={(v) => set({ naming: { ...d.naming, add_tag: v } })} />
          {d.naming.add_tag && (
            <div className="mv-row">
              <label>Tag format</label>
              <input className="mv-input mono" value={d.naming.tag_template} onChange={(e) => set({ naming: { ...d.naming, tag_template: e.target.value } })} placeholder="[{res} {codec}]" />
            </div>
          )}
          <Check label="Add a signature" hint="Appended exactly as typed: _name, [name], -name, whatever you like." checked={d.naming.add_signature} onChange={(v) => set({ naming: { ...d.naming, add_signature: v } })} />
          {d.naming.add_signature && (
            <div className="mv-row">
              <label>Signature</label>
              <input className="mv-input mono" value={d.naming.signature} onChange={(e) => set({ naming: { ...d.naming, signature: e.target.value } })} placeholder="_myname" />
            </div>
          )}
        </Section>

        <Section title="Performance" hint={machine ? `${machine.cpu_brand || "CPU"} · ${machine.physical_cores} cores · ${machine.total_mem_gb} GB · suggested ${suggested} parallel job${suggested === 1 ? "" : "s"} for ≤480p, ${machine.suggested_jobs_hd} for 720p+` : ""}>
          <div className="mv-row">
            <label>Parallel jobs</label>
            <div className="flex items-center gap-2">
              <select className="mv-select" style={{ width: 220 }} value={d.parallel_jobs} onChange={(e) => set({ parallel_jobs: Number(e.target.value) })}>
                <option value={0}>Suggested ({suggested})</option>
                {[1, 2, 3, 4, 5, 6].map((n) => (
                  <option key={n} value={n}>{n}{n === suggested ? " (suggested)" : ""}</option>
                ))}
              </select>
              <button className="mv-btn" onClick={runBench} disabled={benchBusy || !benchJob} title={benchJob ? `Runs 15 s clips of ${benchJob.source.split("/").pop()} at 1, 2 and 3 jobs` : "Add and analyse a file first"}>
                {benchBusy ? <Loader2 size={14} className="spin" /> : null} Benchmark
              </button>
            </div>
          </div>
          {bench && (
            <div className="mb-2 grid grid-cols-3 gap-2">
              {bench.map((b) => (
                <div key={b.jobs} className="rounded-lg p-2.5" style={{ background: "var(--mv-panel-2)" }}>
                  <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{b.jobs} job{b.jobs > 1 ? "s" : ""}</div>
                  <div className="text-[15px] font-medium">{fps(b.total_fps)} fps total</div>
                  <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>{fps(b.per_job_fps)} fps each</div>
                </div>
              ))}
            </div>
          )}
          <div className="mb-2 text-[11.5px]" style={{ color: "var(--mv-faint)" }}>At 360p/480p one ffmpeg process cannot use all your cores, so two or three jobs at once finish a season sooner. The status bar shows total throughput.</div>
          <Check label="Prevent sleep while encoding" checked={d.prevent_sleep} onChange={(v) => set({ prevent_sleep: v })} />
        </Section>

        <Section title="Behaviour">
          <Check label="Start files added while the queue is running right away" hint="Off: new files wait with a Review badge until you start them, so you can change their settings first." checked={d.auto_start_new} onChange={(v) => set({ auto_start_new: v })} />
          <Check label="Desktop notification when the queue finishes" checked={d.notify_on_finish} onChange={(v) => set({ notify_on_finish: v })} />
          <div className="mv-row">
            <label>When the queue ends</label>
            <select className="mv-select" value={d.post_queue_action} onChange={(e) => set({ post_queue_action: e.target.value as AppSettings["post_queue_action"] })}>
              <option value="notify">Nothing else</option>
              <option value="sleep">Put the computer to sleep</option>
              <option value="shutdown">Shut the computer down</option>
            </select>
          </div>
          <div className="mv-row">
            <label>After a crash</label>
            <select className="mv-select" value={d.auto_resume} onChange={(e) => set({ auto_resume: e.target.value as AppSettings["auto_resume"] })}>
              <option value="ask">Show interrupted jobs and ask</option>
              <option value="always">Resume the queue automatically</option>
              <option value="never">Leave them for me to retry</option>
            </select>
          </div>
        </Section>

        <Section title="Default encode settings" hint="Used for every file you add. You can still change each file in the queue.">
          <SettingsPanel value={d.defaults} onChange={(v) => set({ defaults: v })} advanced caps={caps} />
        </Section>

        <Section title="Subtitles" hint="Subtitles next to the video are picked automatically. For OpenSubtitles search you need a free account and an API key (opensubtitles.com → API consumers).">
          <div className="mv-row">
            <label>API key</label>
            <input className="mv-input mono" type="password" value={d.opensubtitles.api_key} onChange={(e) => set({ opensubtitles: { ...d.opensubtitles, api_key: e.target.value } })} />
          </div>
          <div className="mv-row">
            <label>Username</label>
            <input className="mv-input" value={d.opensubtitles.username} onChange={(e) => set({ opensubtitles: { ...d.opensubtitles, username: e.target.value } })} />
          </div>
          <div className="mv-row">
            <label>Password</label>
            <input className="mv-input" type="password" value={d.opensubtitles.password} onChange={(e) => set({ opensubtitles: { ...d.opensubtitles, password: e.target.value } })} />
          </div>
          <div className="mv-row">
            <label>Languages</label>
            <input className="mv-input" placeholder="en,es" value={d.opensubtitles.languages} onChange={(e) => set({ opensubtitles: { ...d.opensubtitles, languages: e.target.value } })} />
          </div>
          <Check label="Save downloaded subtitles next to the video" hint="Otherwise they are kept in the app cache." checked={d.save_subs_next_to_video} onChange={(v) => set({ save_subs_next_to_video: v })} />
        </Section>

        <Section title="ffmpeg" hint="MicroVid ships its own ffmpeg. Point it at another build if you want, for example one with fdk-aac on Windows.">
          <div className="mv-row">
            <label>Custom ffmpeg</label>
            <div className="flex gap-1.5">
              <input className="mv-input mono" placeholder="bundled" value={d.ffmpeg_path ?? ""} onChange={(e) => set({ ffmpeg_path: e.target.value || null })} />
              <button className="mv-btn icon" title="Pick the ffmpeg binary" onClick={async () => { const p = await open({ multiple: false, directory: false }); if (typeof p === "string") set({ ffmpeg_path: p }); }}><FolderOpen size={14} /></button>
              <button className="mv-btn" onClick={redetect}><RefreshCw size={13} /> Detect</button>
            </div>
          </div>
          <div className="text-[12px]" style={{ color: capsError ? "var(--mv-danger)" : "var(--mv-muted)" }}>
            {capsError
              ? capsError
              : caps
                ? `ffmpeg ${caps.version} (${caps.source}) · audio: ${caps.has_aac_at ? "Apple AAC" : caps.has_fdk_aac ? "fdk-aac" : "ffmpeg aac"} · x264 ${caps.has_x264 ? "✓" : "✗"} · x265 ${caps.has_x265 ? "✓" : "✗"} · SVT-AV1 ${caps.has_svtav1 ? "✓" : "✗"} · hardware: ${caps.hw_hevc ?? caps.hw_h264 ?? "none"}`
                : "detecting…"}
          </div>
          <div className="mono mt-1 text-[11px]" style={{ color: "var(--mv-faint)" }}>{caps?.ffmpeg_path}</div>
        </Section>

        <Section title="Appearance and maintenance">
          <div className="mv-row">
            <label>Theme</label>
            <select className="mv-select" style={{ width: 200 }} value={d.theme} onChange={(e) => set({ theme: e.target.value as AppSettings["theme"] })}>
              <option value="system">Follow system</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
          <div className="flex gap-2">
            <button className="mv-btn" onClick={() => ipc.deleteCache().then((n) => showToast(`Freed ${bytes(n)} of test clips and temporary files`, "success"))}>Delete test clips and cache</button>
            <button className="mv-btn danger" onClick={() => ipc.clearHistory().then(() => showToast("History cleared", "success"))}>Clear analytics history</button>
          </div>
        </Section>
      </div>
    </div>
  );
}
