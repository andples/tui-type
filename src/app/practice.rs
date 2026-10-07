//! Missed words and `:pm` (practice missed). Every word with a wrong key
//! in it, fixed or not, is noted as you type and saved per language
//! (`stats::missed_words`) when the test ends or is left. `:pm [n]` types
//! the top `n` of them at random, with no time or word limit: `←`/`→`
//! before the first key steps through `PRACTICE_SIZES`, `tab` starts over,
//! `:pm off` (or `:time`/`:words`) goes back. Practice runs aren't saved as
//! tests, but their misses still count.

use ttyp_core::test::{Mode, Modifiers, RandomGenerator, TestEngine};

use super::App;

/// The top-N choices `←`/`→` cycle through.
pub const PRACTICE_SIZES: [usize; 6] = [1, 3, 5, 10, 25, 50];
/// What `:pm` with no number starts on.
const DEFAULT_SIZE: usize = 10;

impl App {
    /// The language key misses are filed under: the daily's language
    /// during a daily, else the language or custom set being typed.
    fn miss_language(&self) -> String {
        match &self.daily {
            Some(d) => d.language.clone(),
            None => self.language_key(),
        }
    }

    /// After a key: if it was wrong, note the word it went into. `word` and
    /// `keys` are the word index and keystroke count from before the key.
    pub(super) fn note_miss(&mut self, word: usize, keys: usize) {
        let ks = self.engine.keystrokes();
        if ks.len() <= keys || ks.last().is_none_or(|k| k.correct) {
            return;
        }
        let Some(w) = self.engine.words().get(word) else {
            return;
        };
        let target: String = w.target.iter().collect();
        let language = self.miss_language();
        if !self
            .missed_now
            .iter()
            .any(|(l, t)| *l == language && *t == target)
        {
            self.missed_now.push((language, target));
        }
    }

    /// Save this test's missed words.
    pub(super) fn flush_missed_words(&mut self) {
        if self.missed_now.is_empty() {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        for (language, word) in std::mem::take(&mut self.missed_now) {
            self.missed_words.record(&language, [word.as_str()], now);
        }
        if let Err(e) = self.missed_words.save() {
            self.notify(format!("could not save missed words: {e}"));
        }
    }

    /// The next test: practice words while `:pm` is on, else the language
    /// or custom set.
    pub(super) fn fresh_engine(&self) -> TestEngine {
        if let Some(n) = self.practice
            && let Some(engine) = self.practice_engine(n)
        {
            return engine;
        }
        Self::build_engine(&self.config, &self.languages, &self.modules, &self.customs)
    }

    /// Endless: a timed test far longer than anyone types.
    fn practice_engine(&self, n: usize) -> Option<TestEngine> {
        let words: Vec<String> = self
            .missed_words
            .top(&self.language_key(), n)
            .into_iter()
            .map(|(w, _)| w)
            .collect();
        if words.is_empty() {
            return None;
        }
        let generator = RandomGenerator::new(
            words,
            Modifiers {
                punctuation: false,
                numbers: false,
            },
        );
        Some(TestEngine::new(Mode::Time(u16::MAX), Box::new(generator)))
    }

    /// `:pm [n]`: practise the top `n` missed words (the last size, or 10).
    pub(super) fn start_practice(&mut self, n: Option<usize>) {
        if self.refuse_if_daily_locked() {
            return;
        }
        let language = self.language_key();
        let n = n.or(self.practice).unwrap_or(DEFAULT_SIZE).max(1);
        if self.missed_words.count(&language) == 0 {
            self.notify(format!(
                "no missed words for {language} yet · they're collected as you type"
            ));
            return;
        }
        self.practice = Some(n);
        self.restart();
        self.announce_practice();
    }

    pub(super) fn stop_practice(&mut self) {
        if self.practice.take().is_some() {
            self.restart();
            self.notify("practice off");
        }
    }

    /// `←`/`→` before the first key: the next smaller or bigger top-N.
    pub(super) fn practice_step(&mut self, by: i8) {
        let Some(n) = self.practice else { return };
        let i = PRACTICE_SIZES
            .iter()
            .position(|s| *s >= n)
            .unwrap_or(PRACTICE_SIZES.len() - 1);
        let next = (i as isize + by as isize).clamp(0, PRACTICE_SIZES.len() as isize - 1);
        let size = PRACTICE_SIZES[next as usize];
        if size == n {
            return;
        }
        self.practice = Some(size);
        self.restart();
        self.announce_practice();
    }

    fn announce_practice(&mut self) {
        let Some(n) = self.practice else { return };
        let language = self.language_key();
        let top = self.missed_words.top(&language, n);
        const SHOW: usize = 6;
        let mut words = top
            .iter()
            .take(SHOW)
            .map(|(w, e)| format!("{w} ×{}", e.misses))
            .collect::<Vec<_>>()
            .join("  ");
        if top.len() > SHOW {
            words.push_str("  …");
        }
        self.notify(format!("practising {words} · ←→ more or fewer"));
    }

    /// `practice · top 5 of 23 missed` for the mode line.
    pub fn practice_label(&self) -> Option<String> {
        let n = self.practice?;
        let total = self.missed_words.count(&self.language_key());
        Some(format!("practice missed · top {} of {total}", n.min(total)))
    }
}

#[cfg(test)]
mod tests {
    use super::PRACTICE_SIZES;

    #[test]
    fn sizes_start_at_one_word() {
        assert_eq!(PRACTICE_SIZES[0], 1);
        assert!(PRACTICE_SIZES.windows(2).all(|w| w[0] < w[1]));
    }
}
