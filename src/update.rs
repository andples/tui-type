//! Update notice: asks GitHub for the latest release at most once a day,
//! on a background thread, and remembers the answer in `Paths::update_file`
//! so the landing screen can say so from the next start without waiting.
//! Off when the client is offline (`server = ""`) or `update_check = false`.
//! UI-free; `ui::splash` draws the notice.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The latest-release endpoint (`TTYP_RELEASES_URL` at build time for forks).
pub const RELEASES_URL: &str = match option_env!("TTYP_RELEASES_URL") {
    Some(u) => u,
    None => "https://api.github.com/repos/andples/tui-type/releases/latest",
};

/// How long an answer is trusted before asking again.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// `major.minor.patch`; a leading `v` and any `-pre`/`+build` are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('v');
        let core = s.split(['-', '+']).next()?;
        let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
        let v = Version {
            major: parts.next()??,
            minor: parts.next().unwrap_or(Some(0))?,
            patch: parts.next().unwrap_or(Some(0))?,
        };
        parts.next().is_none().then_some(v)
    }

    /// This build's version.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("crate version is semver")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A release newer than this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    pub latest: Version,
    /// The major version went up: shown bigger.
    pub major: bool,
}

impl Update {
    /// `latest` if it's newer than `current`.
    pub fn between(current: Version, latest: Version) -> Option<Self> {
        (latest > current).then_some(Update {
            latest,
            major: latest.major > current.major,
        })
    }
}

/// The last answer, on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    /// Unix seconds of the last successful check.
    pub checked: u64,
    /// The latest release's version then.
    pub latest: String,
}

impl Cache {
    /// A bad cache file is logged, moved aside and read as no answer yet.
    pub fn load(path: &Path) -> Self {
        crate::config::load_state("update", path, |t| serde_json::from_str(t)).unwrap_or_default()
    }

    /// Best effort (logged): without it the next start just asks again.
    pub fn save(&self, path: &Path) {
        let saved = serde_json::to_string(self)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                crate::config::write_atomic(path, text)
                    .map_err(|e| format!("could not save {}: {e}", path.display()))
            });
        crate::errlog::quiet("update", saved);
    }

    pub fn update(&self) -> Option<Update> {
        Update::between(Version::current(), Version::parse(&self.latest)?)
    }

    /// Time to ask again at `now` (unix seconds).
    pub fn due(&self, now: u64) -> bool {
        now.saturating_sub(self.checked) >= CHECK_EVERY.as_secs() || now < self.checked
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The release tag in a GitHub latest-release reply.
pub fn tag_of(json: &str) -> Option<Version> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
    }
    let r: Release = serde_json::from_str(json).ok()?;
    Version::parse(&r.tag_name)
}

/// One background check; `poll` gives its answer once (`None` until then
/// and after). Failures are only logged: the notice is a nicety.
pub struct Checker {
    rx: Receiver<Version>,
}

impl Checker {
    pub fn start(url: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        let url = url.to_string();
        std::thread::spawn(move || {
            let agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(10)))
                .user_agent(concat!("ttyp/", env!("CARGO_PKG_VERSION")))
                .build()
                .new_agent();
            let latest = agent
                .get(&url)
                .header("Accept", "application/vnd.github+json")
                .call()
                .map_err(|e| {
                    let host = crate::online::net::host_of(&url);
                    crate::online::net::explain(&e, host)
                })
                .and_then(|mut r| r.body_mut().read_to_string().map_err(|e| e.to_string()))
                .and_then(|body| tag_of(&body).ok_or_else(|| "no release tag in the reply".into()))
                .map_err(|e| format!("update check {url}: {e}"));
            if let Some(v) = crate::errlog::quiet("update", latest) {
                let _ = tx.send(v);
            }
        });
        Self { rx }
    }

    pub fn poll(&self) -> Poll {
        match self.rx.try_recv() {
            Ok(v) => Poll::Done(v),
            Err(TryRecvError::Empty) => Poll::Waiting,
            Err(TryRecvError::Disconnected) => Poll::Failed,
        }
    }
}

/// Where a `Checker` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    Waiting,
    /// The latest release's version.
    Done(Version),
    /// The check ended without an answer (offline, rate-limited, …).
    Failed,
}

/// How to upgrade, guessed from where the binary lives.
pub fn how_to_upgrade(exe: &Path) -> &'static str {
    let p = exe.to_string_lossy();
    if p.contains("/Cellar/") || p.contains("homebrew") || p.contains("linuxbrew") {
        "brew upgrade ttyp"
    } else if p.contains("/.cargo/bin/") {
        "cargo install ttyp"
    } else if p.starts_with("/usr/bin/") {
        "update ttyp with your package manager"
    } else {
        "github.com/andples/tui-type/releases"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn parses_versions() {
        assert_eq!(
            v("v2.5.0"),
            Version {
                major: 2,
                minor: 5,
                patch: 0
            }
        );
        assert_eq!(v("3.0.0-rc1"), v("3.0.0"));
        assert_eq!(v("3"), v("3.0.0"));
        assert!(Version::parse("x.1").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(v("2.10.0") > v("2.9.9"));
    }

    #[test]
    fn newer_releases_only_and_major_flag() {
        assert_eq!(Update::between(v("2.5.0"), v("2.5.0")), None);
        assert_eq!(Update::between(v("2.5.0"), v("2.4.9")), None);
        let minor = Update::between(v("2.5.0"), v("2.6.1")).unwrap();
        assert!(!minor.major);
        assert!(Update::between(v("2.5.0"), v("3.0.0")).unwrap().major);
    }

    #[test]
    fn reads_the_tag_and_paces_checks() {
        assert_eq!(tag_of(r#"{"tag_name":"v2.6.0","id":1}"#), Some(v("2.6.0")));
        assert_eq!(tag_of("not json"), None);
        let c = Cache {
            checked: 1_000_000,
            latest: "2.6.0".into(),
        };
        assert!(!c.due(1_000_000 + 60));
        assert!(c.due(1_000_000 + CHECK_EVERY.as_secs()));
        assert!(c.due(10), "a clock that went back asks again");
        assert!(Cache::default().due(now_secs()));
    }

    #[test]
    fn upgrade_hint_follows_the_install() {
        let hint = |p: &str| how_to_upgrade(Path::new(p));
        assert_eq!(hint("/opt/homebrew/bin/ttyp"), "brew upgrade ttyp");
        assert_eq!(hint("/home/a/.cargo/bin/ttyp"), "cargo install ttyp");
        assert_eq!(
            hint("/usr/bin/ttyp"),
            "update ttyp with your package manager"
        );
        assert_eq!(hint("/tmp/ttyp"), "github.com/andples/tui-type/releases");
    }
}
