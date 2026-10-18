//! The error log: every error and warning ttyp shows (or has nowhere to
//! show) is appended to `ttyp.log` in the data dir, with a timestamp and
//! where it came from, so a notice that flashed by or a failure on a
//! background thread can still be read afterwards.
//!
//! `init` once at startup; until then (and in tests) entries go nowhere.
//! Logging never fails loudly: a log that can't be written is skipped.

use std::fmt::Display;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The log is moved to `ttyp.log.old` once it grows past this.
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
        }
    }
}

/// An append-only log file, rotated by size.
pub struct Logger {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl Logger {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: Mutex::new(None),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one line: `<time> <LEVEL> [<context>] <message>`.
    pub fn write(&self, level: Level, context: &str, message: &str) {
        let Ok(mut file) = self.file.lock() else {
            return;
        };
        if file.is_none() || self.too_big() {
            *file = self.open();
        }
        let Some(f) = file.as_mut() else { return };
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
        // One entry per line, whatever the message holds.
        let message = message.replace('\n', " ⏎ ");
        if writeln!(f, "{now} {} [{context}] {message}", level.label()).is_err() {
            *file = None;
        }
    }

    fn too_big(&self) -> bool {
        fs::metadata(&self.path).is_ok_and(|m| m.len() > MAX_BYTES)
    }

    fn open(&self) -> Option<File> {
        if self.too_big() {
            let _ = fs::rename(&self.path, self.path.with_extension("log.old"));
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).ok()?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()
    }
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Log to `path` from now on. Later calls are ignored.
pub fn init(path: PathBuf) {
    let _ = LOGGER.set(Logger::new(path));
}

/// Where the log is, once `init` has run.
pub fn path() -> Option<&'static Path> {
    LOGGER.get().map(Logger::path)
}

pub fn log(level: Level, context: &str, message: impl Display) {
    if let Some(l) = LOGGER.get() {
        l.write(level, context, &message.to_string());
    }
}

/// Log an error under `context` (a short tag: `font`, `config`, …) and
/// hand the message back for a notice.
pub fn error(context: &str, msg: impl Into<String>) -> String {
    let msg = msg.into();
    log(Level::Error, context, &msg);
    msg
}

/// Log a warning that is already phrased for the user, and hand it back.
pub fn warn(context: &str, msg: impl Into<String>) -> String {
    let msg = msg.into();
    log(Level::Warn, context, &msg);
    msg
}

/// Log an error that has no notice to go with it (best-effort writes,
/// background threads). Returns the error back as `Option` so it reads as
/// `result.map_err(..)` / `.ok()` at the call site.
pub fn quiet<T, E: Display>(context: &str, result: Result<T, E>) -> Option<T> {
    result.map_err(|e| log(Level::Error, context, e)).ok()
}

/// Run `f`, turning a panic into an `Err` with its message, logged under
/// `context`. For background threads whose answer the UI waits for: a
/// thread that just died would leave it waiting forever.
pub fn catch_panic<T>(context: &str, f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|payload| {
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        error(context, format!("background thread panicked: {msg}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ttyp-errlog-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("ttyp.log")
    }

    #[test]
    fn writes_one_line_per_entry_with_level_and_context() {
        let path = scratch("lines");
        let log = Logger::new(path.clone());
        log.write(Level::Error, "font", "font `x` not found");
        log.write(Level::Warn, "startup", "two\nlines");
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(" ERROR [font] font `x` not found"));
        assert!(lines[1].ends_with(" WARN [startup] two ⏎ lines"));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_big_log_is_moved_aside() {
        let path = scratch("rotate");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        let log = Logger::new(path.clone());
        log.write(Level::Error, "t", "fresh");
        assert!(fs::read_to_string(&path).unwrap().ends_with("fresh\n"));
        assert!(path.with_extension("log.old").is_file());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_panic_becomes_an_error() {
        assert_eq!(catch_panic("t", || 3), Ok(3));
        let e = catch_panic("t", || -> u8 { panic!("boom {}", 1) }).unwrap_err();
        assert!(e.contains("boom 1"), "{e}");
        assert!(
            catch_panic("t", || -> u8 { panic!("static") })
                .unwrap_err()
                .contains("static")
        );
    }

    #[test]
    fn unwritable_log_is_skipped() {
        // A directory where the file should be: nothing panics.
        let path = scratch("dir");
        fs::create_dir_all(&path).unwrap();
        Logger::new(path.clone()).write(Level::Error, "t", "lost");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
