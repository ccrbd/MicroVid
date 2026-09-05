import { useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { BarChart3, FilePlus, FolderPlus, HelpCircle, Pause, Play, Settings } from "lucide-react";
import { ipc } from "../lib/ipc";
import { useStore, type View } from "../lib/store";

const VIDEO_EXTS = ["mkv", "mp4", "m4v", "avi", "mov", "wmv", "ts", "m2ts", "mts", "webm", "flv", "mpg", "mpeg", "vob", "ogv", "3gp"];

export async function addFiles() {
  const picked = await open({ multiple: true, directory: false, filters: [{ name: "Video", extensions: VIDEO_EXTS }] });
  if (!picked) return;
  const paths = Array.isArray(picked) ? picked : [picked];
  await ipc.addSources(paths);
  useStore.getState().setView("queue");
}

export async function addFolder() {
  const picked = await open({ multiple: true, directory: true });
  if (!picked) return;
  const paths = Array.isArray(picked) ? picked : [picked];
  const n = await ipc.addSources(paths);
  useStore.getState().setView("queue");
  if (n === 0) useStore.getState().showToast("No video files found in that folder", "error");
}

export default function Header() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const paused = useStore((s) => s.paused);
  const jobs = useStore((s) => s.jobs);
  const hasWork = jobs.some((j) => ["pending", "probing", "interrupted", "running"].includes(j.status));

  useEffect(() => {
    const f = () => addFiles();
    const d = () => addFolder();
    document.addEventListener("mv:add-files", f);
    document.addEventListener("mv:add-folder", d);
    return () => {
      document.removeEventListener("mv:add-files", f);
      document.removeEventListener("mv:add-folder", d);
    };
  }, []);

  const NavBtn = ({ v, icon, label }: { v: View; icon: React.ReactNode; label: string }) => (
    <button
      className="mv-btn"
      style={view === v ? { background: "var(--mv-panel-2)", borderColor: "var(--mv-accent)" } : undefined}
      onClick={() => setView(view === v && v !== "queue" ? "queue" : v)}
      title={label}
      aria-label={label}
    >
      {icon}
      <span className="hidden lg:inline">{label}</span>
    </button>
  );

  return (
    <div className="flex items-center gap-2 border-b px-3 py-2" style={{ background: "var(--mv-panel)", borderColor: "var(--mv-border)" }} data-tauri-drag-region>
      <button className="flex items-center gap-2" onClick={() => setView("queue")} data-tauri-drag-region>
        <span className="flex h-7 w-7 items-center justify-center rounded-md" style={{ background: "var(--mv-accent)" }}>
          <svg viewBox="0 0 64 64" width="18" height="18">
            <rect x="12" y="18" width="40" height="28" rx="5" fill="none" stroke="#E1F5EE" strokeWidth="5" />
            <path d="M28 26l12 6-12 6z" fill="#E1F5EE" />
            <rect x="20" y="50" width="24" height="4" rx="2" fill="#E1F5EE" />
          </svg>
        </span>
        <span className="text-[14px] font-semibold tracking-tight">MicroVid</span>
      </button>
      <div className="flex-1" data-tauri-drag-region />
      <button className="mv-btn" onClick={addFiles} title="Add files (⌘O)">
        <FilePlus size={15} /> Add files
      </button>
      <button className="mv-btn" onClick={addFolder} title="Add folder (⌘⇧O)">
        <FolderPlus size={15} /> Add folder
      </button>
      <button className={`mv-btn ${paused ? "primary" : ""}`} onClick={() => (paused ? ipc.startQueue() : ipc.pauseQueue())} disabled={paused && !hasWork} title={paused ? "Start queue" : "Pause queue"}>
        {paused ? <Play size={15} /> : <Pause size={15} />}
        {paused ? "Start queue" : "Pause queue"}
      </button>
      <span className="mx-1 h-5 w-px" style={{ background: "var(--mv-border)" }} />
      <NavBtn v="analytics" icon={<BarChart3 size={15} />} label="Analytics" />
      <NavBtn v="settings" icon={<Settings size={15} />} label="Settings" />
      <NavBtn v="help" icon={<HelpCircle size={15} />} label="Help" />
    </div>
  );
}
