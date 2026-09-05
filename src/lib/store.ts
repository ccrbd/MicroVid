import { create } from "zustand";
import type { AppSettings, Capabilities, Job, MachineInfo, QueueStats } from "./types";

export type View = "queue" | "settings" | "help" | "analytics";

interface State {
  jobs: Job[];
  stats: QueueStats | null;
  settings: AppSettings | null;
  machine: MachineInfo | null;
  capabilities: Capabilities | null;
  capabilitiesError: string | null;
  defaultOutputDir: string;
  paused: boolean;
  selectedId: string | null;
  view: View;
  interruptedBanner: number;
  dragging: boolean;
  toast: { text: string; kind: "info" | "error" | "success" } | null;
  setJobs: (jobs: Job[]) => void;
  patchJob: (job: Job) => void;
  setStats: (s: QueueStats) => void;
  setSettings: (s: AppSettings) => void;
  select: (id: string | null) => void;
  setView: (v: View) => void;
  setPaused: (p: boolean) => void;
  setDragging: (d: boolean) => void;
  setInterruptedBanner: (n: number) => void;
  showToast: (text: string, kind?: "info" | "error" | "success") => void;
  init: (p: Partial<State>) => void;
}

export const useStore = create<State>((set) => ({
  jobs: [],
  stats: null,
  settings: null,
  machine: null,
  capabilities: null,
  capabilitiesError: null,
  defaultOutputDir: "",
  paused: true,
  selectedId: null,
  view: "queue",
  interruptedBanner: 0,
  dragging: false,
  toast: null,
  setJobs: (jobs) =>
    set((s) => ({
      jobs,
      selectedId: s.selectedId && jobs.some((j) => j.id === s.selectedId) ? s.selectedId : jobs[0]?.id ?? null,
    })),
  patchJob: (job) => set((s) => ({ jobs: s.jobs.map((j) => (j.id === job.id ? job : j)) })),
  setStats: (stats) => set({ stats, paused: stats.paused }),
  setSettings: (settings) => set({ settings }),
  select: (selectedId) => set({ selectedId }),
  setView: (view) => set({ view }),
  setPaused: (paused) => set({ paused }),
  setDragging: (dragging) => set({ dragging }),
  setInterruptedBanner: (interruptedBanner) => set({ interruptedBanner }),
  showToast: (text, kind = "info") => {
    set({ toast: { text, kind } });
    setTimeout(() => set((s) => (s.toast?.text === text ? { toast: null } : {})), 4500);
  },
  init: (p) => set(p),
}));
