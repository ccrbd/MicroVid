import type { EncodeSettings, Job } from "../lib/types";
import { fileName } from "../lib/format";

interface Props {
  job: Job;
  value: EncodeSettings;
  onChange: (next: EncodeSettings) => void;
  disabled?: boolean;
}

/** Pick which audio / subtitle tracks to keep and which one players should choose by default. */
export default function TrackPicker({ job, value: v, onChange, disabled }: Props) {
  const info = job.info!;
  const audio = info.audio;
  const subs = info.subtitles;
  const externalSub = v.subtitles.mode === "file" ? v.subtitles.file : v.subtitles.mode === "auto" ? job.auto_subtitle : null;

  const keptAudio = (rel: number) =>
    v.audio.mode === "all" ? true : v.audio.mode === "select" ? v.audio.tracks.includes(rel) : (audio.find((a) => a.is_default) ?? audio[0])?.rel_index === rel;
  const toggleAudio = (rel: number) => {
    const cur = audio.filter((a) => keptAudio(a.rel_index)).map((a) => a.rel_index);
    const next = cur.includes(rel) ? cur.filter((x) => x !== rel) : [...cur, rel].sort((a, b) => a - b);
    if (next.length === 0) return;
    const all = next.length === audio.length;
    onChange({ ...v, audio: { ...v.audio, mode: all ? "all" : "select", tracks: all ? [] : next, default_track: v.audio.default_track != null && next.includes(v.audio.default_track) ? v.audio.default_track : null } });
  };
  const audioDefault = v.audio.default_track ?? audio.filter((a) => keptAudio(a.rel_index))[0]?.rel_index ?? null;

  const keptSub = (rel: number) => v.subtitles.keep_source_subs && v.subtitles.mode !== "none" && (v.subtitles.source_tracks == null || v.subtitles.source_tracks.includes(rel));
  const toggleSub = (rel: number) => {
    const cur = subs.filter((s) => keptSub(s.rel_index)).map((s) => s.rel_index);
    const next = cur.includes(rel) ? cur.filter((x) => x !== rel) : [...cur, rel].sort((a, b) => a - b);
    const all = next.length === subs.length;
    onChange({
      ...v,
      subtitles: {
        ...v.subtitles,
        keep_source_subs: next.length > 0 || v.subtitles.keep_source_subs,
        mode: v.subtitles.mode === "none" ? "source" : v.subtitles.mode,
        source_tracks: all ? null : next,
        default_track: v.subtitles.default_track != null && v.subtitles.default_track >= 0 && !next.includes(v.subtitles.default_track) ? null : v.subtitles.default_track,
      },
    });
  };
  const subDefault = v.subtitles.default_track ?? (externalSub ? -1 : null);
  const sub = <T,>(x: T | null | undefined, alt: string) => (x == null || x === "" ? alt : String(x));

  return (
    <div className="mt-2 rounded-lg border p-2.5" style={{ borderColor: "var(--mv-border)" }}>
      <div className="mb-1.5 text-[12px] font-medium" style={{ color: "var(--mv-muted)" }}>Tracks · tick to keep, dot for the default</div>
      {audio.length > 1 && (
        <div className="mb-2">
          <div className="mb-1 text-[11px]" style={{ color: "var(--mv-faint)" }}>Audio</div>
          {audio.map((a) => (
            <label key={a.rel_index} className="flex items-center gap-2 py-0.5 text-[12px]">
              <input type="checkbox" checked={keptAudio(a.rel_index)} disabled={disabled} onChange={() => toggleAudio(a.rel_index)} />
              <input type="radio" name={`adef-${job.id}`} title="Default track" checked={audioDefault === a.rel_index} disabled={disabled || !keptAudio(a.rel_index)} onChange={() => onChange({ ...v, audio: { ...v.audio, default_track: a.rel_index } })} />
              <span className="truncate">
                #{a.rel_index + 1} {sub(a.language, "und")} · {a.codec.toUpperCase()} {a.channel_layout ?? `${a.channels}ch`}{a.title ? ` · ${a.title}` : ""}{a.is_default ? " · source default" : ""}
              </span>
            </label>
          ))}
        </div>
      )}
      {(subs.length > 0 || externalSub) && (
        <div>
          <div className="mb-1 text-[11px]" style={{ color: "var(--mv-faint)" }}>Subtitles</div>
          {externalSub && (
            <label className="flex items-center gap-2 py-0.5 text-[12px]">
              <input type="checkbox" checked readOnly disabled />
              <input type="radio" name={`sdef-${job.id}`} title="Default track" checked={subDefault === -1} disabled={disabled} onChange={() => onChange({ ...v, subtitles: { ...v.subtitles, default_track: -1 } })} />
              <span className="truncate">{fileName(externalSub)} · external file</span>
            </label>
          )}
          {subs.map((s) => (
            <label key={s.rel_index} className="flex items-center gap-2 py-0.5 text-[12px]">
              <input type="checkbox" checked={keptSub(s.rel_index)} disabled={disabled} onChange={() => toggleSub(s.rel_index)} />
              <input type="radio" name={`sdef-${job.id}`} title="Default track" checked={subDefault === s.rel_index} disabled={disabled || !keptSub(s.rel_index)} onChange={() => onChange({ ...v, subtitles: { ...v.subtitles, default_track: s.rel_index } })} />
              <span className="truncate">
                #{s.rel_index + 1} {sub(s.language, "und")} · {s.codec}{s.title ? ` · ${s.title}` : ""}{s.forced ? " · forced" : ""}{!s.text_based ? " · image-based" : ""}
              </span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
