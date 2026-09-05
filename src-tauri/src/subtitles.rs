//! Subtitle discovery next to the video, SRT delay shifting, and the OpenSubtitles REST client.
use crate::models::SubCandidate;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SUB_EXTS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];

fn ext_lower(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default()
}

fn is_sub_file(p: &Path) -> bool {
    let e = ext_lower(p);
    if !SUB_EXTS.contains(&e.as_str()) {
        return false;
    }
    // .sub next to a .idx is VobSub (bitmap) — skip.
    if e == "sub" && p.with_extension("idx").exists() {
        return false;
    }
    true
}

/// Normalise a language token to ISO 639-2 where recognised.
pub fn parse_lang(token: &str) -> Option<String> {
    let t = token.trim().to_ascii_lowercase();
    let t = t.trim_end_matches(".hi").trim_end_matches(".sdh").trim_end_matches(".forced");
    let table: &[(&[&str], &str)] = &[
        (&["en", "eng", "english"], "eng"),
        (&["es", "spa", "spanish", "español"], "spa"),
        (&["fr", "fre", "fra", "french"], "fre"),
        (&["de", "ger", "deu", "german"], "ger"),
        (&["it", "ita", "italian"], "ita"),
        (&["pt", "por", "portuguese", "pt-br"], "por"),
        (&["ru", "rus", "russian"], "rus"),
        (&["ja", "jpn", "japanese"], "jpn"),
        (&["ko", "kor", "korean"], "kor"),
        (&["zh", "chi", "zho", "chinese"], "chi"),
        (&["ar", "ara", "arabic"], "ara"),
        (&["hi", "hin", "hindi"], "hin"),
        (&["bn", "ben", "bengali", "bangla"], "ben"),
        (&["tr", "tur", "turkish"], "tur"),
        (&["nl", "dut", "nld", "dutch"], "dut"),
        (&["sv", "swe", "swedish"], "swe"),
        (&["pl", "pol", "polish"], "pol"),
        (&["id", "ind", "indonesian"], "ind"),
        (&["th", "tha", "thai"], "tha"),
        (&["vi", "vie", "vietnamese"], "vie"),
    ];
    for (keys, code) in table {
        if keys.contains(&t) {
            return Some((*code).to_string());
        }
    }
    None
}

/// Extract a language from a subtitle stem given the video stem ("movie.en.srt" → eng).
fn lang_from_suffix(video_stem: &str, sub_stem: &str) -> Option<String> {
    let rest = sub_stem.strip_prefix(video_stem)?.trim_start_matches(['.', '_', '-', ' ']);
    if rest.is_empty() {
        return None;
    }
    rest.split(['.', '_', '-', ' ']).find_map(parse_lang)
}

fn push_dir(dir: &Path, video_stem_lc: &str, base_score: u32, any_match: bool, source: &str, out: &mut Vec<SubCandidate>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() || !is_sub_file(&p) {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let stem_lc = stem.to_ascii_lowercase();
        let (score, lang) = if stem_lc == video_stem_lc {
            (base_score, None)
        } else if stem_lc.starts_with(video_stem_lc) && stem_lc[video_stem_lc.len()..].starts_with(['.', '_', '-', ' ']) {
            (base_score - 10, lang_from_suffix(video_stem_lc, &stem_lc))
        } else if any_match {
            (base_score - 30, stem_lc.split(['.', '_', '-', ' ']).find_map(parse_lang))
        } else {
            continue;
        };
        out.push(SubCandidate { path: p.to_string_lossy().into_owned(), language: lang, source: source.into(), score });
    }
}

/// Find subtitle files for a video: same folder, then Subs/Subtitles folders.
pub fn find_candidates(video: &Path) -> Vec<SubCandidate> {
    let mut out = vec![];
    let Some(dir) = video.parent() else { return out };
    let stem_lc = video.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    push_dir(dir, &stem_lc, 100, false, "same-name", &mut out);

    // Is this the only video in the folder? Then any sub in Subs/ is probably for it.
    let only_video = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| crate::probe::is_video_file(&e.path())).count() <= 1)
        .unwrap_or(false);
    for name in ["Subs", "subs", "Subtitles", "subtitles", "SUBS", "Sub"] {
        let sd = dir.join(name);
        if sd.is_dir() {
            push_dir(&sd, &stem_lc, 80, only_video, "subs-folder", &mut out);
            // Subs/<video stem>/*.srt convention
            if let Ok(rd) = std::fs::read_dir(&sd) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() && p.file_name().and_then(|n| n.to_str()).map(|n| n.to_ascii_lowercase() == stem_lc).unwrap_or(false) {
                        push_dir(&p, &stem_lc, 75, true, "subs-folder", &mut out);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

pub fn best_candidate<'a>(cands: &'a [SubCandidate], preferred_lang: &str) -> Option<&'a SubCandidate> {
    cands.iter().max_by_key(|c| {
        let lang_bonus = match &c.language {
            Some(l) if l == preferred_lang => 15,
            None => 5, // unknown language is more likely to be "the" subtitle than a wrong language
            _ => 0,
        };
        c.score + lang_bonus
    })
}

/// Shift every SRT timestamp by `delay_ms` (negative moves earlier; never below zero).
pub fn shift_srt(input: &str, delay_ms: i64) -> String {
    let re = regex::Regex::new(r"(\d{1,2}):(\d{2}):(\d{2})[,.](\d{1,3})").unwrap();
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        if line.contains("-->") {
            let shifted = re.replace_all(line, |c: &regex::Captures| {
                let h: i64 = c[1].parse().unwrap_or(0);
                let m: i64 = c[2].parse().unwrap_or(0);
                let s: i64 = c[3].parse().unwrap_or(0);
                let ms_str = &c[4];
                let ms: i64 = format!("{:0<3}", ms_str).parse().unwrap_or(0);
                let total = (((h * 60 + m) * 60 + s) * 1000 + ms + delay_ms).max(0);
                format!("{:02}:{:02}:{:02},{:03}", total / 3_600_000, (total / 60_000) % 60, (total / 1000) % 60, total % 1000)
            });
            out.push_str(&shifted);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// OpenSubtitles "moviehash": file size + sum of first and last 64 KiB as little-endian u64s.
pub fn movie_hash(path: &Path) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let size = f.metadata()?.len();
    let chunk = 65536u64.min(size);
    let mut hash: u64 = size;
    let mut buf = vec![0u8; chunk as usize];
    f.read_exact(&mut buf)?;
    for c in buf.chunks_exact(8) {
        hash = hash.wrapping_add(u64::from_le_bytes(c.try_into().unwrap()));
    }
    f.seek(SeekFrom::Start(size.saturating_sub(chunk)))?;
    f.read_exact(&mut buf)?;
    for c in buf.chunks_exact(8) {
        hash = hash.wrapping_add(u64::from_le_bytes(c.try_into().unwrap()));
    }
    Ok(format!("{hash:016x}"))
}

// ---------------- OpenSubtitles REST API v1 ----------------

const OS_BASE: &str = "https://api.opensubtitles.com/api/v1";
const USER_AGENT: &str = "MicroVid v0.1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OsSearchResult {
    pub file_id: u64,
    pub file_name: String,
    pub language: String,
    pub release: String,
    pub download_count: u64,
    pub fps: Option<f64>,
    pub hearing_impaired: bool,
    pub from_trusted: bool,
    pub moviehash_match: bool,
    pub title: String,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(USER_AGENT).timeout(std::time::Duration::from_secs(30)).build()?)
}

pub async fn os_login(api_key: &str, username: &str, password: &str) -> Result<String> {
    let c = client()?;
    let resp = c
        .post(format!("{OS_BASE}/login"))
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .context("contacting OpenSubtitles")?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("login failed ({status}): {}", body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error")));
    }
    body.get("token").and_then(|t| t.as_str()).map(|s| s.to_string()).ok_or_else(|| anyhow!("no token in login response"))
}

pub async fn os_search(api_key: &str, query: &str, languages: &str, moviehash: Option<&str>, file_size: Option<u64>) -> Result<Vec<OsSearchResult>> {
    let c = client()?;
    let mut params: Vec<(String, String)> = vec![("languages".into(), languages.to_string()), ("order_by".into(), "download_count".into())];
    if !query.trim().is_empty() {
        params.push(("query".into(), query.trim().to_string()));
    }
    if let Some(h) = moviehash {
        params.push(("moviehash".into(), h.to_string()));
    }
    let resp = c.get(format!("{OS_BASE}/subtitles")).header("Api-Key", api_key).query(&params).send().await.context("contacting OpenSubtitles")?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("search failed ({status}): {}", body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error")));
    }
    let _ = file_size;
    let mut out = vec![];
    for item in body.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
        let attr = item.get("attributes").cloned().unwrap_or_default();
        let files = attr.get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default();
        let Some(file) = files.first() else { continue };
        let Some(file_id) = file.get("file_id").and_then(|x| x.as_u64()) else { continue };
        out.push(OsSearchResult {
            file_id,
            file_name: file.get("file_name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            language: attr.get("language").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            release: attr.get("release").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            download_count: attr.get("download_count").and_then(|x| x.as_u64()).unwrap_or(0),
            fps: attr.get("fps").and_then(|x| x.as_f64()),
            hearing_impaired: attr.get("hearing_impaired").and_then(|x| x.as_bool()).unwrap_or(false),
            from_trusted: attr.get("from_trusted").and_then(|x| x.as_bool()).unwrap_or(false),
            moviehash_match: attr.get("moviehash_match").and_then(|x| x.as_bool()).unwrap_or(false),
            title: attr.get("feature_details").and_then(|f| f.get("title")).and_then(|x| x.as_str()).unwrap_or("").to_string(),
        });
    }
    Ok(out)
}

/// Download a subtitle file and write it to `dest_dir/<video stem>.<lang>.srt`.
pub async fn os_download(api_key: &str, token: &str, file_id: u64, dest: &Path) -> Result<PathBuf> {
    let c = client()?;
    let resp = c
        .post(format!("{OS_BASE}/download"))
        .header("Api-Key", api_key)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "file_id": file_id, "sub_format": "srt" }))
        .send()
        .await
        .context("requesting download link")?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("download failed ({status}): {}", body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error")));
    }
    let link = body.get("link").and_then(|l| l.as_str()).ok_or_else(|| anyhow!("no download link returned"))?;
    let bytes = c.get(link).send().await?.bytes().await?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &bytes)?;
    Ok(dest.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_parsing() {
        assert_eq!(parse_lang("en"), Some("eng".into()));
        assert_eq!(parse_lang("English"), Some("eng".into()));
        assert_eq!(parse_lang("pt-br"), Some("por".into()));
        assert_eq!(parse_lang("xx"), None);
        assert_eq!(lang_from_suffix("movie", "movie.en"), Some("eng".into()));
        assert_eq!(lang_from_suffix("movie", "movie.forced.eng"), Some("eng".into()));
        assert_eq!(lang_from_suffix("movie", "movie"), None);
    }

    #[test]
    fn finds_same_name_and_subs_folder() {
        let dir = std::env::temp_dir().join(format!("microvid-subs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("Subs")).unwrap();
        std::fs::write(dir.join("Show S01E01.mkv"), b"").unwrap();
        std::fs::write(dir.join("Show S01E01.srt"), b"").unwrap();
        std::fs::write(dir.join("Show S01E01.es.srt"), b"").unwrap();
        std::fs::write(dir.join("Show S01E02.srt"), b"").unwrap();
        std::fs::write(dir.join("Subs").join("show s01e01.eng.srt"), b"").unwrap();
        std::fs::write(dir.join("Subs").join("2_English.srt"), b"").unwrap();
        let c = find_candidates(&dir.join("Show S01E01.mkv"));
        let paths: Vec<String> = c.iter().map(|x| Path::new(&x.path).file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(paths[0], "Show S01E01.srt");
        assert!(paths.contains(&"Show S01E01.es.srt".to_string()));
        assert!(paths.contains(&"show s01e01.eng.srt".to_string()));
        assert!(paths.contains(&"2_English.srt".to_string()), "only video in folder → any sub in Subs/ counts");
        assert!(!paths.contains(&"Show S01E02.srt".to_string()));
        let best = best_candidate(&c, "eng").unwrap();
        assert!(best.path.ends_with("Show S01E01.srt"));
        let es = c.iter().find(|x| x.path.ends_with(".es.srt")).unwrap();
        assert_eq!(es.language.as_deref(), Some("spa"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shifts_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\nHi\n\n2\n00:00:00,200 --> 00:00:00,900\nEarly\n";
        let out = shift_srt(srt, -500);
        assert!(out.contains("00:00:00,500 --> 00:00:02,000"));
        assert!(out.contains("00:00:00,000 --> 00:00:00,400"));
        let out2 = shift_srt(srt, 3600_000);
        assert!(out2.contains("01:00:01,000 --> 01:00:02,500"));
    }
}
