import { useEffect } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { ipc, onEvent } from "./lib/ipc";
import { useStore } from "./lib/store";
import type { Job, QueueStats } from "./lib/types";
import { bytes } from "./lib/format";
import Header from "./components/Header";
import StatusBar from "./components/StatusBar";
import QueueView from "./views/QueueView";
import SettingsView from "./views/SettingsView";
import HelpView from "./views/HelpView";
import AnalyticsView from "./views/AnalyticsView";
import { AlertTriangle, X } from "lucide-react";

async function notify(title: string, body: string) {
  try {
    let ok = await isPermissionGranted();
    if (!ok) ok = (await requestPermission()) === "granted";
    if (ok) sendNotification({ title, body });
  } catch {
    /* notifications unavailable */
  }
}

export default function App() {
  const view = useStore((s) => s.view);
  const settings = useStore((s) => s.settings);
  const toast = useStore((s) => s.toast);
  const dragging = useStore((s) => s.dragging);
  const interrupted = useStore((s) => s.interruptedBanner);

  useEffect(() => {
    const st = useStore.getState();
    ipc.startupInfo().then((info) => {
      st.init({
        jobs: info.jobs,
        settings: info.settings,
        machine: info.machine,
        capabilities: info.capabilities,
        capabilitiesError: info.capabilities_error,
        defaultOutputDir: info.default_output_dir,
        paused: info.paused,
        selectedId: info.jobs[0]?.id ?? null,
        interruptedBanner: info.interrupted,
      });
      if (info.interrupted > 0 && info.settings.auto_resume === "always") {
        ipc.startQueue();
        st.setInterruptedBanner(0);
      }
    });
    const unlisten: Array<Promise<() => void>> = [
      onEvent<Job[]>("queue:changed", (jobs) => useStore.getState().setJobs(jobs)),
      onEvent<Job>("job:progress", (job) => useStore.getState().patchJob(job)),
      onEvent<QueueStats>("queue:stats", (stats) => useStore.getState().setStats(stats)),
      onEvent<{ done: number; failed: number; in_bytes: number; out_bytes: number }>("queue:finished", (s) => {
        const text = `Queue finished: ${s.done} done${s.failed ? `, ${s.failed} failed` : ""} · ${bytes(s.in_bytes)} → ${bytes(s.out_bytes)}`;
        useStore.getState().showToast(text, s.failed ? "error" : "success");
        const cfg = useStore.getState().settings;
        if (cfg?.notify_on_finish) notify("MicroVid", text);
      }),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") useStore.getState().setDragging(true);
        else if (p.type === "leave") useStore.getState().setDragging(false);
        else if (p.type === "drop") {
          useStore.getState().setDragging(false);
          useStore.getState().setView("queue");
          ipc.addSources(p.paths).then((n) => {
            if (n === 0) useStore.getState().showToast("No new video files found in what you dropped", "error");
          });
        }
      }),
    ];
    return () => {
      unlisten.forEach((u) => u.then((f) => f()));
    };
  }, []);

  useEffect(() => {
    const theme = settings?.theme ?? "system";
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && mq.matches);
      document.documentElement.classList.toggle("dark", dark);
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [settings?.theme]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      const s = useStore.getState();
      if (mod && e.key.toLowerCase() === "o") {
        e.preventDefault();
        document.dispatchEvent(new CustomEvent(e.shiftKey ? "mv:add-folder" : "mv:add-files"));
      } else if (mod && e.key === ",") {
        e.preventDefault();
        s.setView("settings");
      } else if (e.key === "Escape") {
        s.setView("queue");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-full flex-col">
      <Header />
      {interrupted > 0 && (
        <div className="flex items-center gap-3 border-b px-4 py-2 text-[12.5px]" style={{ background: "var(--mv-warn-soft)", color: "var(--mv-warn-text)", borderColor: "var(--mv-border)" }}>
          <AlertTriangle size={15} />
          <span className="flex-1">
            {interrupted} job{interrupted > 1 ? "s were" : " was"} interrupted by a crash or shutdown. Partial files were removed; press Start to encode them again.
          </span>
          <button className="mv-btn primary" style={{ height: 26 }} onClick={() => { ipc.startQueue(); useStore.getState().setInterruptedBanner(0); }}>
            Resume queue
          </button>
          <button className="mv-btn icon" style={{ height: 26, width: 26 }} onClick={() => useStore.getState().setInterruptedBanner(0)} aria-label="Dismiss">
            <X size={14} />
          </button>
        </div>
      )}
      <div className="relative min-h-0 flex-1">
        {view === "queue" && <QueueView />}
        {view === "settings" && <SettingsView />}
        {view === "help" && <HelpView />}
        {view === "analytics" && <AnalyticsView />}
        {dragging && (
          <div className="pointer-events-none absolute inset-0 z-40 flex items-center justify-center" style={{ background: "color-mix(in srgb, var(--mv-accent) 12%, transparent)" }}>
            <div className="rounded-xl border-2 border-dashed px-8 py-6 text-lg font-medium" style={{ borderColor: "var(--mv-accent)", background: "var(--mv-panel)", color: "var(--mv-accent-text)" }}>
              Drop to add to the queue
            </div>
          </div>
        )}
        {toast && (
          <div
            className="absolute bottom-3 left-1/2 z-50 -translate-x-1/2 rounded-lg border px-4 py-2 text-[12.5px] shadow-md"
            style={{
              background: toast.kind === "error" ? "var(--mv-danger-soft)" : toast.kind === "success" ? "var(--mv-accent-soft)" : "var(--mv-panel)",
              color: toast.kind === "error" ? "var(--mv-danger)" : toast.kind === "success" ? "var(--mv-accent-text)" : "var(--mv-text)",
              borderColor: "var(--mv-border)",
            }}
          >
            {toast.text}
          </div>
        )}
      </div>
      <StatusBar />
    </div>
  );
}
