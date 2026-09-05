import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Analytics, AppSettings, BenchmarkPoint, Capabilities, EncodeSettings, Estimate, Job, NamingSettings,
  OsSearchResult, QueueStats, StartupInfo, TestEncodeResult,
} from "./types";

export const ipc = {
  startupInfo: () => invoke<StartupInfo>("startup_info"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<AppSettings>("set_settings", { settings }),
  getCapabilities: () => invoke<Capabilities>("get_capabilities"),
  addSources: (paths: string[]) => invoke<number>("add_sources", { paths }),
  listJobs: () => invoke<Job[]>("list_jobs"),
  updateJobSettings: (ids: string[], settings: EncodeSettings) => invoke<Job[]>("update_job_settings", { ids, settings }),
  estimateSettings: (id: string, settings: EncodeSettings) => invoke<Estimate>("estimate_settings", { id, settings }),
  previewOutputName: (naming: NamingSettings, settings: EncodeSettings) => invoke<string>("preview_output_name", { naming, settings }),
  removeJobs: (ids: string[]) => invoke<void>("remove_jobs", { ids }),
  clearFinished: () => invoke<void>("clear_finished"),
  retryJobs: (ids: string[]) => invoke<void>("retry_jobs", { ids }),
  cancelJobs: (ids: string[]) => invoke<void>("cancel_jobs", { ids }),
  reorderJobs: (ids: string[]) => invoke<void>("reorder_jobs", { ids }),
  startQueue: () => invoke<void>("start_queue"),
  pauseQueue: () => invoke<void>("pause_queue"),
  queueStats: () => invoke<QueueStats>("queue_stats"),
  refreshSubtitles: (id: string) => invoke<Job>("refresh_subtitles", { id }),
  osSearch: (id: string, query?: string, languages?: string) => invoke<OsSearchResult[]>("opensubtitles_search", { args: { id, query: query ?? null, languages: languages ?? null } }),
  osDownload: (id: string, fileId: number, language: string) => invoke<Job>("opensubtitles_download", { id, fileId, language }),
  testEncode: (id: string, startSecs?: number) => invoke<TestEncodeResult>("test_encode", { id, startSecs: startSecs ?? null }),
  benchmark: (id: string, maxJobs: number) => invoke<BenchmarkPoint[]>("benchmark", { id, maxJobs }),
  getAnalytics: () => invoke<Analytics>("get_analytics"),
  clearHistory: () => invoke<void>("clear_history"),
  exportHistoryCsv: (path: string) => invoke<number>("export_history_csv", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  deleteCache: () => invoke<number>("delete_cache"),
  pathExists: (path: string) => invoke<boolean>("path_exists", { path }),
};

export function onEvent<T>(name: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(name, (e) => handler(e.payload));
}
