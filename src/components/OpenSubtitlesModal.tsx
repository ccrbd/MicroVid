import { useEffect, useState } from "react";
import { Download, Loader2, Search } from "lucide-react";
import Modal from "./Modal";
import { ipc } from "../lib/ipc";
import { useStore } from "../lib/store";
import type { Job, OsSearchResult } from "../lib/types";
import { fileName } from "../lib/format";

export default function OpenSubtitlesModal({ job, onClose }: { job: Job; onClose: () => void }) {
  const settings = useStore((s) => s.settings);
  const showToast = useStore((s) => s.showToast);
  const [query, setQuery] = useState(fileName(job.source).replace(/\.[^.]+$/, ""));
  const [langs, setLangs] = useState(settings?.opensubtitles.languages || "en");
  const [busy, setBusy] = useState(false);
  const [downloading, setDownloading] = useState<number | null>(null);
  const [results, setResults] = useState<OsSearchResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const configured = !!settings?.opensubtitles.api_key;

  const search = async () => {
    setBusy(true);
    setError(null);
    try {
      setResults(await ipc.osSearch(job.id, query, langs));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    if (configured) search();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const download = async (r: OsSearchResult) => {
    setDownloading(r.file_id);
    try {
      await ipc.osDownload(job.id, r.file_id, r.language);
      showToast(`Subtitle downloaded and selected: ${r.file_name}`, "success");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setDownloading(null);
    }
  };

  return (
    <Modal title="Search OpenSubtitles" onClose={onClose} width={760}>
      {!configured && (
        <div className="mb-3 rounded-lg p-3 text-[12px]" style={{ background: "var(--mv-warn-soft)", color: "var(--mv-warn-text)" }}>
          Add your OpenSubtitles API key and login in Settings → Subtitles first. A free account at opensubtitles.com gives you an API key under “API consumers”.
          <button className="mv-btn ml-2" style={{ height: 24 }} onClick={() => { onClose(); useStore.getState().setView("settings"); }}>Open settings</button>
        </div>
      )}
      <div className="mb-3 flex gap-2">
        <input className="mv-input" value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={(e) => e.key === "Enter" && search()} placeholder="Title, e.g. The Wire S01E03" />
        <input className="mv-input" style={{ width: 110 }} value={langs} onChange={(e) => setLangs(e.target.value)} title="Comma separated language codes, e.g. en,es" />
        <button className="mv-btn primary" onClick={search} disabled={busy || !configured}>
          {busy ? <Loader2 size={14} className="spin" /> : <Search size={14} />} Search
        </button>
      </div>
      <div className="mb-2 text-[11.5px]" style={{ color: "var(--mv-faint)" }}>The file's hash is sent too, so exact matches for your release are flagged.</div>
      {error && <div className="mb-3 text-[12px]" style={{ color: "var(--mv-danger)" }}>{error}</div>}
      {results && results.length === 0 && <div className="text-[12px]" style={{ color: "var(--mv-muted)" }}>Nothing found. Try a shorter title.</div>}
      {results && results.length > 0 && (
        <table className="w-full text-[12px]">
          <thead>
            <tr style={{ color: "var(--mv-faint)" }}>
              <th className="py-1 text-left font-normal">Release</th>
              <th className="py-1 text-left font-normal">Lang</th>
              <th className="py-1 text-right font-normal">Downloads</th>
              <th className="py-1 text-right font-normal">fps</th>
              <th className="py-1"></th>
            </tr>
          </thead>
          <tbody>
            {results.map((r) => (
              <tr key={r.file_id} className="border-t" style={{ borderColor: "var(--mv-border)" }}>
                <td className="py-1.5 pr-2">
                  <div className="truncate" style={{ maxWidth: 380 }} title={r.file_name}>{r.release || r.file_name}</div>
                  <div className="text-[11px]" style={{ color: "var(--mv-faint)" }}>
                    {r.title}{r.moviehash_match ? " · exact match for your file" : ""}{r.hearing_impaired ? " · HI" : ""}{r.from_trusted ? " · trusted" : ""}
                  </div>
                </td>
                <td>{r.language}</td>
                <td className="text-right">{r.download_count.toLocaleString()}</td>
                <td className="text-right">{r.fps ?? "–"}</td>
                <td className="text-right">
                  <button className="mv-btn" style={{ height: 24 }} disabled={downloading != null} onClick={() => download(r)}>
                    {downloading === r.file_id ? <Loader2 size={13} className="spin" /> : <Download size={13} />} Use
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Modal>
  );
}
