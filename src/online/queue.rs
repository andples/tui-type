//! Daily results that couldn't be submitted, kept in the data dir and
//! retried on the next start (only while the daily's UTC day lasts).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ttyp_core::api::SubmitRequest;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Queued {
    /// UTC date of the daily, `YYYY-MM-DD`.
    pub date: String,
    pub request: SubmitRequest,
}

/// Write one queued submission; the name keeps entries unique and ordered.
pub fn save(dir: &Path, q: &Queued) -> io::Result<PathBuf> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("{}-{ms}.json", q.request.daily_id));
    let json = serde_json::to_vec(q).map_err(io::Error::other)?;
    crate::config::write_atomic(&path, json)?;
    Ok(path)
}

/// Every readable entry, oldest first. A file that can't be read is
/// logged and moved aside (`<file>.bad`), so it isn't retried forever.
pub fn load(dir: &Path) -> Vec<(PathBuf, Queued)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let q = crate::config::load_state("queue", &p, |t| serde_json::from_str(t))?;
            Some((p, q))
        })
        .collect()
}

pub fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_files_from_before_starts_still_load() {
        let old = r#"{"date":"2026-09-29","request":{"daily_id":7,"keylog":[]}}"#;
        let q: Queued = serde_json::from_str(old).unwrap();
        assert_eq!(q.request.start_id, None);
    }

    #[test]
    fn queue_round_trip() {
        let dir = std::env::temp_dir().join(format!("ttyp-queue-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(load(&dir).is_empty());
        let q = Queued {
            date: "2026-09-29".into(),
            request: SubmitRequest {
                daily_id: 7,
                keylog: vec![],
                start_id: Some(3),
            },
        };
        let path = save(&dir, &q).unwrap();
        fs::write(dir.join("junk.json"), b"not json").unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded, vec![(path.clone(), q)]);
        // The junk is moved aside, not tried again on every start.
        assert!(!dir.join("junk.json").exists());
        assert_eq!(fs::read(dir.join("junk.json.bad")).unwrap(), b"not json");
        assert_eq!(load(&dir).len(), 1);
        remove(&path).unwrap();
        remove(&path).unwrap();
        assert!(load(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
