//! SQLite persistence: queue, history (analytics), calibration and settings.
use crate::models::{AppSettings, CalibrationRow, HistoryRow, Job, JobStatus};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK (id = 1), json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, status TEXT NOT NULL, ord INTEGER NOT NULL, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT, output TEXT, codec TEXT, resolution INTEGER, crf INTEGER,
                content_type TEXT, in_size INTEGER, out_size INTEGER, duration_secs REAL, elapsed_secs REAL, avg_fps REAL,
                avg_speed REAL, finished_at INTEGER, parallel_jobs INTEGER);
             CREATE TABLE IF NOT EXISTS calibration (key TEXT PRIMARY KEY, throughput_fps REAL NOT NULL, samples INTEGER NOT NULL);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn load_settings(&self) -> AppSettings {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT json FROM settings WHERE id = 1", [], |r| r.get::<_, String>(0))
            .optional()
            .ok()
            .flatten()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, s: &AppSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO settings (id, json) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET json = excluded.json", params![serde_json::to_string(s)?])?;
        Ok(())
    }

    pub fn load_jobs(&self) -> Vec<Job> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT json FROM jobs ORDER BY ord ASC") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| r.get::<_, String>(0))
            .map(|rows| rows.flatten().filter_map(|j| serde_json::from_str::<Job>(&j).ok()).collect())
            .unwrap_or_default()
    }

    pub fn upsert_job(&self, job: &Job) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO jobs (id, status, ord, json) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET status = excluded.status, ord = excluded.ord, json = excluded.json",
            params![job.id, job.status.as_str(), job.order, serde_json::to_string(job)?],
        )?;
        Ok(())
    }

    pub fn upsert_jobs(&self, jobs: &[Job]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for job in jobs {
            tx.execute(
                "INSERT INTO jobs (id, status, ord, json) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET status = excluded.status, ord = excluded.ord, json = excluded.json",
                params![job.id, job.status.as_str(), job.order, serde_json::to_string(job)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_job(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_history(&self, job: &Job, parallel_jobs: u32) -> Result<()> {
        if job.status != JobStatus::Done {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let (res, dur) = job
            .estimate
            .as_ref()
            .map(|e| (e.out_height, 0.0))
            .map(|(h, _)| (h, job.info.as_ref().map(|i| i.duration_secs).unwrap_or(0.0)))
            .unwrap_or((0, job.info.as_ref().map(|i| i.duration_secs).unwrap_or(0.0)));
        conn.execute(
            "INSERT INTO history (source, output, codec, resolution, crf, content_type, in_size, out_size, duration_secs, elapsed_secs, avg_fps, avg_speed, finished_at, parallel_jobs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                job.source,
                job.output,
                if job.settings.hardware { format!("{} (hw)", job.settings.codec.label()) } else { job.settings.codec.label().to_string() },
                res,
                job.settings.effective_crf(),
                serde_json::to_value(job.settings.content_type).ok().and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
                job.in_size as i64,
                job.out_size.unwrap_or(0) as i64,
                dur,
                job.elapsed_secs.unwrap_or(0.0),
                job.avg_fps.unwrap_or(0.0),
                job.avg_speed.unwrap_or(0.0),
                job.finished_at.unwrap_or(0),
                parallel_jobs
            ],
        )?;
        Ok(())
    }

    pub fn list_history(&self, limit: usize) -> Vec<HistoryRow> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, source, output, codec, resolution, crf, content_type, in_size, out_size, duration_secs, elapsed_secs, avg_fps, avg_speed, finished_at, parallel_jobs
             FROM history ORDER BY finished_at DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![limit as i64], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                source: r.get(1)?,
                output: r.get(2)?,
                codec: r.get(3)?,
                resolution: r.get::<_, i64>(4)? as u32,
                crf: r.get::<_, i64>(5)? as u8,
                content_type: r.get(6)?,
                in_size: r.get::<_, i64>(7)? as u64,
                out_size: r.get::<_, i64>(8)? as u64,
                duration_secs: r.get(9)?,
                elapsed_secs: r.get(10)?,
                avg_fps: r.get(11)?,
                avg_speed: r.get(12)?,
                finished_at: r.get(13)?,
                parallel_jobs: r.get::<_, i64>(14)? as u32,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn calibration(&self, key: &str) -> Option<(f64, u32)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT throughput_fps, samples FROM calibration WHERE key = ?1", params![key], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, i64>(1)? as u32)))
            .optional()
            .ok()
            .flatten()
    }

    /// Rolling average over at most 8 samples so the estimate adapts to changes.
    pub fn update_calibration(&self, key: &str, throughput_fps: f64) -> Result<()> {
        if !(throughput_fps.is_finite() && throughput_fps > 0.0) {
            return Ok(());
        }
        let existing = self.calibration(key);
        let (avg, n) = match existing {
            Some((old, n)) => {
                let n2 = n.min(7) + 1;
                ((old * (n2 - 1) as f64 + throughput_fps) / n2 as f64, n2)
            }
            None => (throughput_fps, 1),
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO calibration (key, throughput_fps, samples) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET throughput_fps = excluded.throughput_fps, samples = excluded.samples",
            params![key, avg, n as i64],
        )?;
        Ok(())
    }

    pub fn list_calibration(&self) -> Vec<CalibrationRow> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT key, throughput_fps, samples FROM calibration ORDER BY key") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| Ok(CalibrationRow { key: r.get(0)?, throughput_fps: r.get(1)?, samples: r.get::<_, i64>(2)? as u32 }))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_jobs_settings_and_calibration() {
        let dir = std::env::temp_dir().join(format!("microvid-db-{}", uuid::Uuid::new_v4()));
        let db = Db::open(&dir.join("t.db")).unwrap();
        let mut s = AppSettings::default();
        s.parallel_jobs = 3;
        db.save_settings(&s).unwrap();
        assert_eq!(db.load_settings().parallel_jobs, 3);

        let mut j = Job::default();
        j.id = "a".into();
        j.source = "/x.mkv".into();
        j.status = JobStatus::Pending;
        db.upsert_job(&j).unwrap();
        j.status = JobStatus::Done;
        j.out_size = Some(10);
        j.in_size = 100;
        j.finished_at = Some(5);
        db.upsert_job(&j).unwrap();
        let jobs = db.load_jobs();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, JobStatus::Done);
        db.add_history(&j, 2).unwrap();
        let h = db.list_history(10);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].out_size, 10);
        db.delete_job("a").unwrap();
        assert!(db.load_jobs().is_empty());

        db.update_calibration("hevc:slower:480", 100.0).unwrap();
        db.update_calibration("hevc:slower:480", 50.0).unwrap();
        let (v, n) = db.calibration("hevc:slower:480").unwrap();
        assert_eq!(n, 2);
        assert!((v - 75.0).abs() < 1e-9);
        std::fs::remove_dir_all(&dir).ok();
    }
}
