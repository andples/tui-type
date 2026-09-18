//! Test history. `StatsStore` is the seam for a future server backend; the
//! only implementation today appends JSON lines to a local file.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::test::mode::Mode;
use crate::test::{CharCounts, Metrics};

#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharRecord {
    pub correct: usize,
    pub incorrect: usize,
    pub extra: usize,
    pub missed: usize,
}

impl From<CharCounts> for CharRecord {
    fn from(c: CharCounts) -> Self {
        Self {
            correct: c.correct,
            incorrect: c.incorrect,
            extra: c.extra,
            missed: c.missed,
        }
    }
}

/// One completed test. `schema` lets future versions migrate old lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestRecord {
    pub schema: u8,
    pub ts: DateTime<Utc>,
    pub mode: Mode,
    pub language: String,
    pub punctuation: bool,
    pub numbers: bool,
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub chars: CharRecord,
    pub duration_s: f64,
}

impl TestRecord {
    pub const SCHEMA: u8 = 1;

    pub fn new(
        metrics: &Metrics,
        mode: Mode,
        language: &str,
        punctuation: bool,
        numbers: bool,
    ) -> Self {
        Self {
            schema: Self::SCHEMA,
            ts: Utc::now(),
            mode,
            language: language.to_string(),
            punctuation,
            numbers,
            wpm: metrics.wpm,
            raw: metrics.raw,
            acc: metrics.accuracy,
            consistency: metrics.consistency,
            chars: metrics.chars.into(),
            duration_s: metrics.duration.as_secs_f64(),
        }
    }
}

pub trait StatsStore {
    fn append(&mut self, record: &TestRecord) -> Result<(), StatsError>;
    /// All records, oldest first.
    fn all(&self) -> &[TestRecord];
}

/// Appends one JSON object per line to `history.jsonl`; keeps everything in
/// memory after the initial load.
pub struct LocalJsonlStore {
    path: PathBuf,
    records: Vec<TestRecord>,
}

impl LocalJsonlStore {
    /// Load existing history. Unparsable lines are skipped and counted so a
    /// corrupt line never blocks startup.
    pub fn open(path: &Path) -> Result<(Self, usize), StatsError> {
        let mut records = Vec::new();
        let mut skipped = 0;
        match fs::File::open(path) {
            Ok(f) => {
                for line in BufReader::new(f).lines() {
                    let line = line.map_err(|source| StatsError::Io {
                        path: path.to_path_buf(),
                        source,
                    })?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str(&line) {
                        Ok(r) => records.push(r),
                        Err(_) => skipped += 1,
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StatsError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
        Ok((
            Self {
                path: path.to_path_buf(),
                records,
            },
            skipped,
        ))
    }
}

impl StatsStore for LocalJsonlStore {
    fn append(&mut self, record: &TestRecord) -> Result<(), StatsError> {
        let io = |source| StatsError::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io)?;
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        f.write_all(line.as_bytes()).map_err(io)?;
        self.records.push(record.clone());
        Ok(())
    }

    fn all(&self) -> &[TestRecord] {
        &self.records
    }
}

/// Aggregates shown on the stats screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub tests: usize,
    pub avg_wpm: f64,
    pub avg_acc: f64,
    pub recent_avg_wpm: f64,
    pub recent_avg_acc: f64,
    /// Best wpm per mode (all languages).
    pub best: HashMap<Mode, f64>,
}

impl Summary {
    pub fn from_records(records: &[TestRecord]) -> Self {
        let n = records.len();
        if n == 0 {
            return Self::default();
        }
        let avg = |rs: &[TestRecord], f: fn(&TestRecord) -> f64| {
            rs.iter().map(f).sum::<f64>() / rs.len() as f64
        };
        let recent = &records[n.saturating_sub(10)..];
        let mut best: HashMap<Mode, f64> = HashMap::new();
        for r in records {
            let e = best.entry(r.mode).or_insert(0.0);
            if r.wpm > *e {
                *e = r.wpm;
            }
        }
        Self {
            tests: n,
            avg_wpm: avg(records, |r| r.wpm),
            avg_acc: avg(records, |r| r.acc),
            recent_avg_wpm: avg(recent, |r| r.wpm),
            recent_avg_acc: avg(recent, |r| r.acc),
            best,
        }
    }
}

/// Personal best for a (mode, language) pair, excluding `exclude_last` most
/// recent records (so the just-finished test can be compared against history).
pub fn personal_best(records: &[TestRecord], mode: Mode, language: &str) -> Option<f64> {
    records
        .iter()
        .filter(|r| r.mode == mode && r.language == language)
        .map(|r| r.wpm)
        .fold(None, |acc, w| Some(acc.map_or(w, |a: f64| a.max(w))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(mode: Mode, wpm: f64) -> TestRecord {
        TestRecord {
            schema: 1,
            ts: Utc::now(),
            mode,
            language: "english".into(),
            punctuation: false,
            numbers: false,
            wpm,
            raw: wpm,
            acc: 95.0,
            consistency: 70.0,
            chars: CharRecord {
                correct: 1,
                incorrect: 0,
                extra: 0,
                missed: 0,
            },
            duration_s: 30.0,
        }
    }

    #[test]
    fn jsonl_roundtrip_and_skip_bad_lines() {
        let dir = std::env::temp_dir().join(format!("ttyp-stats-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        let (mut store, skipped) = LocalJsonlStore::open(&path).unwrap();
        assert_eq!(skipped, 0);
        store.append(&rec(Mode::Time(30), 80.0)).unwrap();
        store.append(&rec(Mode::Words(25), 90.0)).unwrap();
        fs::write(
            &path,
            format!("{}garbage\n", fs::read_to_string(&path).unwrap()),
        )
        .unwrap();
        let (store, skipped) = LocalJsonlStore::open(&path).unwrap();
        assert_eq!(store.all().len(), 2);
        assert_eq!(skipped, 1);
        assert_eq!(store.all()[1].mode, Mode::Words(25));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn summary_and_pb() {
        let rs: Vec<_> = (0..12)
            .map(|i| {
                rec(
                    if i % 2 == 0 {
                        Mode::Time(30)
                    } else {
                        Mode::Time(15)
                    },
                    50.0 + i as f64,
                )
            })
            .collect();
        let s = Summary::from_records(&rs);
        assert_eq!(s.tests, 12);
        assert_eq!(s.best[&Mode::Time(30)], 60.0);
        assert_eq!(s.best[&Mode::Time(15)], 61.0);
        assert!((s.recent_avg_wpm - 56.5).abs() < 1e-9);
        assert_eq!(personal_best(&rs, Mode::Time(30), "english"), Some(60.0));
        assert_eq!(personal_best(&rs, Mode::Words(10), "english"), None);
        assert_eq!(Summary::from_records(&[]), Summary::default());
    }
}
