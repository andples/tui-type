//! The update notice's app side: the cached answer at start, a background
//! check when the last one is a day old, and saving what it finds.
//! `crate::update` holds the rules; `ui::splash` draws `App::update`.

use super::App;
use crate::config::Config;
use crate::update::{self, Cache, Checker, Poll};

impl App {
    /// Checks are on: online (`server` not empty) and not turned off.
    fn update_checks_on(config: &Config) -> bool {
        config.update_check && config.server.as_deref().is_none_or(|s| !s.is_empty())
    }

    /// At start: show what the last check found, and ask again if it's due.
    pub(super) fn start_update_check(&mut self) {
        if !Self::update_checks_on(&self.config) {
            return;
        }
        let cache = Cache::load(&self.paths.update_file);
        self.update = cache.update();
        if cache.due(update::now_secs()) {
            self.update_checker = Some(Checker::start(update::RELEASES_URL));
        }
    }

    /// Take the background check's answer when it has arrived.
    pub(super) fn drain_update(&mut self) {
        let Some(checker) = &self.update_checker else {
            return;
        };
        match checker.poll() {
            Poll::Waiting => {}
            Poll::Done(latest) => {
                self.update_checker = None;
                let cache = Cache {
                    checked: update::now_secs(),
                    latest: latest.to_string(),
                };
                cache.save(&self.paths.update_file);
                self.update = cache.update();
                self.dirty = true;
            }
            Poll::Failed => self.update_checker = None,
        }
    }
}
