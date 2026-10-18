//! Account sync: while logged in, the configs (files and which are on) and
//! the personal bests follow the account to every machine. Configs are
//! fetched at start, on login, on opening `:config` and every
//! `PULL_EVERY` after a key, and pushed after every local change; a reply
//! from the server is merged three ways (`sync::Configs::merge`) with the
//! last agreed state (`App::synced`) and applied once no test is running.
//! Bests are sent at start, on login, with each later fetch, on opening the
//! stats and after each new best; the server keeps the fastest and the
//! reply (all of them) is cached in `App::bests`.

use std::time::{Duration, Instant};

use ttyp_core::sync::{Best, ConfigPush, ConfigSync, Configs};
use ttyp_core::test::{Mode, Status};

use super::{App, Screen};
use crate::online::sync::{self, Synced};
use crate::online::{OnlineError, Request};
use crate::profile::{ProfileMenu, ProfileRegistry};
use crate::stats::{Summary, personal_best};

/// How often configs are fetched again while ttyp is in use.
const PULL_EVERY: Duration = Duration::from_secs(60);

impl App {
    fn logged_in(&self) -> bool {
        self.online.as_ref().is_some_and(|o| o.logged_in())
    }

    fn sync_request(&mut self, req: Request) {
        if let Some(o) = &mut self.online
            && o.logged_in()
        {
            o.request(req);
        }
    }

    /// At start and on login: fetch the configs, and send the bests the
    /// account doesn't have yet (the reply is all of them).
    pub(super) fn start_sync(&mut self) {
        if !self.logged_in() {
            return;
        }
        self.pull_configs();
        self.refresh_bests();
    }

    /// Send the bests the account doesn't have yet; the reply is all of
    /// them, so this also picks up bests set on other machines.
    pub(super) fn refresh_bests(&mut self) {
        let missing = self.bests.missing(self.stats.all());
        self.sync_request(Request::PushBests(missing));
    }

    /// The fastest counted run on `mode` in `language`, here or on any
    /// machine logged in to the same account.
    pub fn best(&self, mode: Mode, language: &str) -> Option<f64> {
        let local = personal_best(self.stats.all(), mode, language);
        match (local, self.bests.get(mode, language)) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }

    /// A new best was just set here.
    pub(super) fn push_best(&mut self, mode: Mode, language: &str, wpm: f64) {
        let best = Best {
            mode,
            language: language.to_string(),
            wpm,
        };
        if best.problem().is_none() {
            self.sync_request(Request::PushBests(vec![best]));
        }
    }

    pub(super) fn bests_reply(&mut self, result: Result<Vec<Best>, OnlineError>) {
        // Quietly on failure: they're sent again with the next fetch.
        let Ok(list) = result else {
            return;
        };
        if let Err(e) = self.bests.set(list) {
            self.report("sync", format!("could not save bests: {e}"));
        }
        if self.screen == Screen::Stats {
            self.summary = Summary::from_records(self.stats.all()).with_bests(self.bests.by_mode());
        }
    }

    pub(super) fn pull_configs(&mut self) {
        if self.config_sync_busy || !self.logged_in() {
            return;
        }
        self.config_sync_busy = true;
        self.last_config_pull = Some(Instant::now());
        self.sync_request(Request::Configs);
    }

    fn local_configs(&self) -> Configs {
        sync::local_configs(&self.paths.profiles_dir, &self.config.profiles)
    }

    /// After any change to the configs or which are on: send them unless
    /// the server already has them. Before the first sync, fetch instead,
    /// so the first push is a merge.
    pub(super) fn configs_changed(&mut self) {
        if self.config_sync_busy || !self.logged_in() {
            return;
        }
        let Some(synced) = &self.synced else {
            self.pull_configs();
            return;
        };
        let local = self.local_configs();
        if local != synced.configs {
            let push = ConfigPush {
                base_rev: synced.rev,
                configs: local,
            };
            self.config_sync_busy = true;
            self.sync_request(Request::PushConfigs(push));
        }
    }

    pub(super) fn configs_reply(
        &mut self,
        pushed: Option<ConfigPush>,
        result: Result<ConfigSync, OnlineError>,
    ) {
        self.config_sync_busy = false;
        let remote = match result {
            Ok(r) => r,
            Err(OnlineError::Unauthorized) => return,
            Err(e) => {
                if self.screen == Screen::Profiles {
                    self.report("sync", format!("config sync: {e}"));
                }
                return;
            }
        };
        match pushed {
            Some(p) if remote.rev == p.base_rev + 1 && remote.configs == p.configs => {
                self.set_synced(Synced {
                    rev: remote.rev,
                    configs: p.configs,
                });
                // Anything changed while the push was out goes next.
                self.configs_changed();
            }
            // A fetch, or a push that lost to another machine's: merge.
            _ => {
                self.pending_configs = Some(remote);
                self.sync_tick();
            }
        }
    }

    /// Every turn of the event loop: apply fetched configs once nothing is
    /// in the way, and fetch again now and then.
    pub(super) fn sync_tick(&mut self) {
        let in_test = self.engine.status() == Status::Running || self.daily.is_some();
        let editing =
            self.screen == Screen::Profiles && matches!(self.profile_menu, ProfileMenu::Edit(_));
        if in_test || editing {
            return;
        }
        if let Some(remote) = self.pending_configs.take() {
            self.merge_configs(remote);
        } else if self
            .last_config_pull
            .is_some_and(|t| t.elapsed() >= PULL_EVERY)
            && self.last_input.elapsed() < PULL_EVERY
        {
            self.pull_configs();
            self.refresh_bests();
        }
    }

    /// Bring this machine and the server together after fetching `remote`.
    fn merge_configs(&mut self, remote: ConfigSync) {
        if self.synced.as_ref().is_some_and(|s| s.rev == remote.rev) {
            // Nothing new there; send anything new here.
            self.configs_changed();
            return;
        }
        let local = self.local_configs();
        let merged = Configs::merge(
            self.synced.as_ref().map(|s| &s.configs),
            &local,
            &remote.configs,
        );
        // The server's state is the base from here; whatever this machine
        // ends up with that differs from it is pushed by `configs_changed`.
        self.set_synced(Synced {
            rev: remote.rev,
            configs: remote.configs,
        });
        if merged != local {
            self.apply_configs(&local, &merged);
        }
        self.configs_changed();
    }

    /// Write `to`'s config files and switch on its active configs.
    fn apply_configs(&mut self, from: &Configs, to: &Configs) {
        if let Err(e) = sync::write_configs(&self.paths.profiles_dir, from, to) {
            self.report("sync", format!("config sync: {e}"));
            return;
        }
        let mut warnings = Vec::new();
        self.profiles = ProfileRegistry::load(&self.paths.profiles_dir, |w| warnings.push(w));
        let before = self.config.clone();
        self.config.profiles.clear();
        for name in &to.active {
            if let Err(e) = self.profiles.activate(name, &mut self.config) {
                warnings.push(e);
            }
        }
        self.refresh_profile_completions();
        if self.screen == Screen::Profiles {
            let latest = self.config.profiles.last().cloned();
            self.profile_menu = ProfileMenu::list_at(&self.profiles, latest.as_deref());
        }
        match (warnings.first(), self.config.profiles.as_slice()) {
            (Some(w), _) => self.notify(format!("config sync: {w}")),
            (None, []) => self.notify("configs synced"),
            (None, on) => self.notify(format!("configs synced · {} on", on.join(", "))),
        }
        self.apply_changes(&before);
        self.install_missing();
        self.save_config();
    }

    fn set_synced(&mut self, synced: Synced) {
        if let Err(e) = synced.save(&self.paths.sync_file) {
            self.report("sync", format!("config sync: {e}"));
        }
        self.synced = Some(synced);
    }

    /// Logging out: forget the account's synced state on this machine.
    /// The config files and this machine's history stay.
    pub(super) fn forget_sync(&mut self) {
        self.synced = None;
        self.pending_configs = None;
        self.config_sync_busy = false;
        let done = sync::forget(&self.paths.sync_file).and_then(|()| self.bests.clear());
        if let Err(e) = done {
            self.report("sync", format!("could not forget synced state: {e}"));
        }
    }
}
