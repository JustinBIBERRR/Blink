use crate::state::SessionOutcome;
use rusqlite::{Connection, Result};
use serde::Serialize;

pub struct Db {
    pub conn: Connection,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatsToday {
    pub completed: u32,
    pub early_exit: u32,
    pub skipped: u32,
    pub warn_skipped: u32,
    pub total_sessions: u32,
    pub compliance_pct: u32,
}

#[derive(Debug, Serialize, Clone)]
pub struct DayStats {
    pub date: String,
    pub completed: u32,
    pub early_exit: u32,
    pub skipped: u32,
    pub warn_skipped: u32,
}

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                outcome   TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_timestamp ON sessions(timestamp);",
        )?;
        Ok(Self { conn })
    }

    pub fn record_session(&self, outcome: &SessionOutcome) -> Result<()> {
        let ts = chrono::Utc::now().timestamp();
        let outcome_str = match outcome {
            SessionOutcome::Completed => "completed",
            SessionOutcome::EarlyExit => "early_exit",
            SessionOutcome::Skipped => "skipped",
            SessionOutcome::WarnSkipped => "warn_skipped",
        };
        self.conn.execute(
            "INSERT INTO sessions (timestamp, outcome) VALUES (?1, ?2)",
            rusqlite::params![ts, outcome_str],
        )?;
        Ok(())
    }

    pub fn get_stats_today(&self) -> Result<StatsToday> {
        let today_start = {
            use chrono::{Datelike, Local, TimeZone};
            let now = Local::now();
            Local
                .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                .unwrap()
                .timestamp()
        };
        self.aggregate_range(today_start, i64::MAX)
            .map(|(c, e, s, w)| {
                let total = c + e + s + w;
                let compliance_pct = if total == 0 { 0 } else { (c * 100) / total };
                StatsToday {
                    completed: c,
                    early_exit: e,
                    skipped: s,
                    warn_skipped: w,
                    total_sessions: total,
                    compliance_pct,
                }
            })
    }

    pub fn get_stats_week(&self) -> Result<Vec<DayStats>> {
        self.get_day_stats(7)
    }

    pub fn get_stats_month(&self) -> Result<Vec<DayStats>> {
        self.get_day_stats(30)
    }

    pub fn cleanup_old_records(&self) -> Result<()> {
        let cutoff = chrono::Utc::now().timestamp() - (90 * 86400);
        self.conn
            .execute("DELETE FROM sessions WHERE timestamp < ?1", [cutoff])?;
        Ok(())
    }

    fn get_day_stats(&self, days: i64) -> Result<Vec<DayStats>> {
        use chrono::{Datelike, Duration, Local, TimeZone};
        let mut result = Vec::new();
        let today = Local::now();
        for i in 0..days {
            let day = today.date_naive() - Duration::days(days - 1 - i);
            let day_start = Local
                .with_ymd_and_hms(day.year(), day.month(), day.day(), 0, 0, 0)
                .unwrap()
                .timestamp();
            let day_end = day_start + 86400;
            let (completed, early_exit, skipped, warn_skipped) =
                self.aggregate_range(day_start, day_end)?;
            result.push(DayStats {
                date: day.format("%Y-%m-%d").to_string(),
                completed,
                early_exit,
                skipped,
                warn_skipped,
            });
        }
        Ok(result)
    }

    fn aggregate_range(&self, from: i64, to: i64) -> Result<(u32, u32, u32, u32)> {
        let mut stmt = self.conn.prepare(
            "SELECT outcome, COUNT(*) FROM sessions WHERE timestamp >= ?1 AND timestamp < ?2 GROUP BY outcome",
        )?;
        let mut completed = 0u32;
        let mut early_exit = 0u32;
        let mut skipped = 0u32;
        let mut warn_skipped = 0u32;
        let rows = stmt.query_map(rusqlite::params![from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;
        for row in rows {
            let (outcome, count) = row?;
            match outcome.as_str() {
                "completed" => completed = count,
                "early_exit" => early_exit = count,
                "skipped" => skipped = count,
                "warn_skipped" => warn_skipped = count,
                _ => {}
            }
        }
        Ok((completed, early_exit, skipped, warn_skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SessionOutcome;
    use tempfile::tempdir;

    fn test_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let path_str = path.to_str().unwrap().to_string();
        std::mem::forget(dir);
        Db::open(&path_str).unwrap()
    }

    #[test]
    fn init_creates_table() {
        let db = test_db();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn record_and_fetch_today() {
        let db = test_db();
        db.record_session(&SessionOutcome::Completed).unwrap();
        db.record_session(&SessionOutcome::Skipped).unwrap();
        let stats = db.get_stats_today().unwrap();
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.total_sessions, 2);
    }

    #[test]
    fn cleanup_removes_90_day_old_records() {
        let db = test_db();
        let old_ts = chrono::Utc::now().timestamp() - (100 * 86400);
        db.conn
            .execute(
                "INSERT INTO sessions (timestamp, outcome) VALUES (?1, ?2)",
                rusqlite::params![old_ts, "completed"],
            )
            .unwrap();
        db.record_session(&SessionOutcome::Completed).unwrap();
        db.cleanup_old_records().unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
