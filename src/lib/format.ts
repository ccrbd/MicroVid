export function bytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !isFinite(n)) return "–";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : digits)} ${units[i]}`;
}

export function duration(secs: number | null | undefined, compact = false): string {
  if (secs == null || !isFinite(secs)) return "–";
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (compact) {
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${r}s`;
    return `${r}s`;
  }
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
  return `${m}:${String(r).padStart(2, "0")}`;
}

export function percentSaved(inB: number, outB: number): string {
  if (!inB || !outB) return "–";
  return `${Math.round((1 - outB / inB) * 100)}%`;
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function dirName(path: string): string {
  const parts = path.split(/[\\/]/);
  parts.pop();
  return parts.join("/");
}

export function fps(n: number | null | undefined): string {
  if (n == null || !isFinite(n)) return "–";
  return n >= 100 ? n.toFixed(0) : n.toFixed(1);
}

export const codecLabel: Record<string, string> = { x264: "H.264 (x264)", hevc: "HEVC (x265)", av1: "AV1 (SVT-AV1)" };
export const codecShort: Record<string, string> = { x264: "x264", hevc: "HEVC", av1: "AV1" };
export const contentLabel: Record<string, string> = {
  general: "General",
  drama: "Drama / film",
  sitcom: "Sitcom / TV",
  animation: "Animation / cartoon",
  action: "Action / sports",
  news: "News / talk",
};
export const resolutionLabel: Record<string, string> = { "360": "360p", "480": "480p", "576": "576p", "720": "720p", "1080": "1080p", source: "Keep original" };
