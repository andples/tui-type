//! Difficulty: `sudden_death` (a wrong key fails the test) and `min_wpm`
//! (dropping under a speed fails it, checked from `MinWpm::GRACE_SECS`
//! in). Both are left out of dailies, so every daily runs the same rules.
//! A failed run is ended at once and kept in the history as invalid
//! (`Invalid::SuddenDeath`, `Invalid::BelowMinWpm`), so it never sets a best;
//! one that fails within its first second just starts over, unsaved.

use std::time::{Duration, Instant};

use super::App;
use crate::config::MinWpm;
use crate::stats::validity::Invalid;
use crate::test::{Metrics, Status};

impl App {
    /// The minimum speed is on and this test is subject to it.
    pub(super) fn min_wpm_applies(&self) -> bool {
        self.config.min_wpm.is_on() && self.daily.is_none()
    }

    /// End the running test if it just failed a difficulty setting.
    /// Returns whether it did.
    pub(super) fn check_difficulty(&mut self, now: Instant) -> bool {
        if self.daily.is_some() || self.engine.status() != Status::Running {
            return false;
        }
        let Some(why) = self.failure_at(now) else {
            return false;
        };
        // Too short to score (a wrong first key): start over instead of
        // saving a run with nonsense speeds.
        if self.engine.elapsed_at(now) < Duration::from_secs(1) {
            self.restart();
            self.notify(format!("failed · {} · new words", why.label()));
            return true;
        }
        self.engine.end_at(now);
        self.failed = Some(why);
        self.finish_test();
        true
    }

    fn failure_at(&self, now: Instant) -> Option<Invalid> {
        let e = &self.engine;
        if self.config.sudden_death && e.keystrokes().iter().any(|k| !k.correct) {
            return Some(Invalid::SuddenDeath);
        }
        if !self.min_wpm_applies() {
            return None;
        }
        let elapsed = e.elapsed_at(now);
        if elapsed < Duration::from_secs(MinWpm::GRACE_SECS) {
            return None;
        }
        let wpm = Metrics::compute(e.words(), e.current_index(), e.keystrokes(), elapsed).wpm;
        (wpm < f64::from(self.config.min_wpm.0)).then_some(Invalid::BelowMinWpm)
    }
}
