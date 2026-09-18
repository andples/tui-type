//! The typing test state machine. Pure logic: takes characters and instants,
//! never touches the terminal. All time-dependent methods have an `_at`
//! variant that accepts an `Instant` so tests are deterministic.

use std::time::{Duration, Instant};

use super::generator::WordGenerator;
use super::mode::Mode;

/// How many words to keep queued ahead of the caret in Time mode.
const LOOKAHEAD: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Words shown, timer not started.
    Idle,
    Running,
    Finished,
}

/// One logged character keystroke (backspaces are not logged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keystroke {
    /// Time since the test started.
    pub at: Duration,
    pub correct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub target: Vec<char>,
    pub typed: Vec<char>,
}

impl Word {
    fn new(s: &str) -> Self {
        Self {
            target: s.chars().collect(),
            typed: Vec::new(),
        }
    }

    pub fn is_correct(&self) -> bool {
        self.typed == self.target
    }

    pub fn has_error(&self) -> bool {
        self.typed.iter().zip(&self.target).any(|(t, e)| t != e)
            || self.typed.len() > self.target.len()
    }
}

pub struct TestEngine {
    mode: Mode,
    generator: Box<dyn WordGenerator>,
    words: Vec<Word>,
    current: usize,
    status: Status,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
    keystrokes: Vec<Keystroke>,
}

impl TestEngine {
    pub fn new(mode: Mode, mut generator: Box<dyn WordGenerator>) -> Self {
        let n = match mode {
            Mode::Words(n) => n as usize,
            Mode::Time(_) => LOOKAHEAD,
        };
        let words = generator
            .next_words(n)
            .iter()
            .map(|w| Word::new(w))
            .collect();
        Self {
            mode,
            generator,
            words,
            current: 0,
            status: Status::Idle,
            started_at: None,
            finished_at: None,
            keystrokes: Vec::new(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn words(&self) -> &[Word] {
        &self.words
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    pub fn current_word(&self) -> &Word {
        &self.words[self.current]
    }

    pub fn keystrokes(&self) -> &[Keystroke] {
        &self.keystrokes
    }

    pub fn is_finished(&self) -> bool {
        self.status == Status::Finished
    }

    /// Time elapsed at `now` (zero before the test starts, frozen after finish).
    pub fn elapsed_at(&self, now: Instant) -> Duration {
        match (self.started_at, self.finished_at) {
            (Some(s), Some(f)) => f.duration_since(s),
            (Some(s), None) => now.duration_since(s),
            _ => Duration::ZERO,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed_at(Instant::now())
    }

    /// Remaining time in Time mode, `None` in Words mode.
    pub fn remaining_at(&self, now: Instant) -> Option<Duration> {
        match self.mode {
            Mode::Time(secs) => {
                Some(Duration::from_secs(secs as u64).saturating_sub(self.elapsed_at(now)))
            }
            Mode::Words(_) => None,
        }
    }

    /// Words completed so far (for the `n/total` counter in Words mode).
    pub fn words_completed(&self) -> usize {
        self.current
    }

    /// Advance the clock. Finishes a Time-mode test whose limit has passed.
    /// Returns `true` if the test just finished.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.status != Status::Running {
            return false;
        }
        if let Mode::Time(secs) = self.mode
            && self.elapsed_at(now) >= Duration::from_secs(secs as u64)
        {
            self.finish(now);
            return true;
        }
        false
    }

    pub fn type_char(&mut self, c: char) {
        self.type_char_at(c, Instant::now());
    }

    pub fn type_char_at(&mut self, c: char, now: Instant) {
        if self.status == Status::Finished {
            return;
        }
        if self.status == Status::Idle {
            // Leading space does nothing; the first real character starts the clock.
            if c == ' ' {
                return;
            }
            self.status = Status::Running;
            self.started_at = Some(now);
        }
        // Time limit may have passed between ticks.
        if self.tick(now) {
            return;
        }
        let at = self.elapsed_at(now);

        if c == ' ' {
            if self.words[self.current].typed.is_empty() {
                return;
            }
            let correct = self.words[self.current].is_correct();
            self.keystrokes.push(Keystroke { at, correct });
            self.advance(now);
            return;
        }

        let word = &mut self.words[self.current];
        let correct = word.target.get(word.typed.len()) == Some(&c);
        word.typed.push(c);
        self.keystrokes.push(Keystroke { at, correct });

        // Words mode ends the moment the last word is typed correctly.
        if matches!(self.mode, Mode::Words(_))
            && self.current + 1 == self.words.len()
            && self.words[self.current].is_correct()
        {
            self.finish(now);
        }
    }

    fn advance(&mut self, now: Instant) {
        if self.current + 1 >= self.words.len() {
            // Space after the last word in Words mode ends the test (with the
            // last word marked wrong if it was).
            self.finish(now);
            return;
        }
        self.current += 1;
        self.ensure_lookahead();
    }

    /// Time mode: keep enough words queued that the screen never runs dry.
    fn ensure_lookahead(&mut self) {
        if !matches!(self.mode, Mode::Time(_)) {
            return;
        }
        let ahead = self.words.len() - self.current;
        if ahead < LOOKAHEAD / 2 {
            let more = self.generator.next_words(LOOKAHEAD);
            self.words.extend(more.iter().map(|w| Word::new(w)));
        }
    }

    /// Delete one character. Moves back into the previous word only if that
    /// word was left with an error (matching monkeytype's default).
    pub fn backspace(&mut self) {
        if self.status == Status::Finished {
            return;
        }
        if self.words[self.current].typed.pop().is_some() {
            return;
        }
        if self.can_go_back() {
            self.current -= 1;
        }
    }

    /// Ctrl+Backspace / Ctrl+W: clear the current word, or if it is already
    /// empty, go back to the previous erroneous word and clear that.
    pub fn delete_word(&mut self) {
        if self.status == Status::Finished {
            return;
        }
        if self.words[self.current].typed.is_empty() && self.can_go_back() {
            self.current -= 1;
        }
        self.words[self.current].typed.clear();
    }

    fn can_go_back(&self) -> bool {
        self.current > 0 && !self.words[self.current - 1].is_correct()
    }

    fn finish(&mut self, now: Instant) {
        self.status = Status::Finished;
        self.finished_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<&'static str>);
    impl WordGenerator for Fixed {
        fn next_words(&mut self, n: usize) -> Vec<String> {
            self.0
                .iter()
                .cycle()
                .take(n)
                .map(|s| s.to_string())
                .collect()
        }
    }

    fn engine(mode: Mode) -> TestEngine {
        TestEngine::new(mode, Box::new(Fixed(vec!["ab", "cd", "ef"])))
    }

    fn type_str(e: &mut TestEngine, s: &str, t0: Instant) {
        for (i, c) in s.chars().enumerate() {
            e.type_char_at(c, t0 + Duration::from_millis(100 * i as u64));
        }
    }

    #[test]
    fn words_mode_finishes_on_last_char() {
        let mut e = engine(Mode::Words(3));
        let t0 = Instant::now();
        assert_eq!(e.status(), Status::Idle);
        type_str(&mut e, "ab cd e", t0);
        assert_eq!(e.status(), Status::Running);
        e.type_char_at('f', t0 + Duration::from_secs(1));
        assert_eq!(e.status(), Status::Finished);
        assert_eq!(e.elapsed(), Duration::from_secs(1));
        assert!(e.keystrokes().iter().all(|k| k.correct));
    }

    #[test]
    fn leading_space_ignored_and_does_not_start() {
        let mut e = engine(Mode::Words(3));
        e.type_char(' ');
        assert_eq!(e.status(), Status::Idle);
        e.type_char('a');
        e.type_char(' ');
        assert_eq!(e.current_index(), 1);
        // Space on an empty word is a no-op.
        e.type_char(' ');
        assert_eq!(e.current_index(), 1);
    }

    #[test]
    fn backspace_only_into_erroneous_word() {
        let mut e = engine(Mode::Words(3));
        type_str(&mut e, "ab ", Instant::now());
        assert_eq!(e.current_index(), 1);
        e.backspace();
        assert_eq!(e.current_index(), 1, "previous word correct: stay");

        let mut e = engine(Mode::Words(3));
        type_str(&mut e, "ax ", Instant::now());
        e.backspace();
        assert_eq!(e.current_index(), 0, "previous word wrong: go back");
        assert_eq!(e.current_word().typed, vec!['a', 'x']);
        e.backspace();
        assert_eq!(e.current_word().typed, vec!['a']);
    }

    #[test]
    fn extra_chars_and_error_flags() {
        let mut e = engine(Mode::Words(3));
        type_str(&mut e, "abzz", Instant::now());
        let w = e.current_word();
        assert!(w.has_error());
        assert!(!w.is_correct());
        let ks = e.keystrokes();
        assert_eq!(ks.iter().filter(|k| k.correct).count(), 2);
        assert_eq!(ks.len(), 4);
    }

    #[test]
    fn delete_word() {
        let mut e = engine(Mode::Words(3));
        type_str(&mut e, "ax cd", Instant::now());
        e.delete_word();
        assert!(e.current_word().typed.is_empty());
        e.delete_word();
        assert_eq!(e.current_index(), 0);
        assert!(e.current_word().typed.is_empty());
    }

    #[test]
    fn time_mode_tops_up_and_finishes_on_tick() {
        let mut e = engine(Mode::Time(15));
        let t0 = Instant::now();
        let initial = e.words().len();
        for i in 0..initial - 5 {
            type_str(&mut e, "ab ", t0 + Duration::from_millis(i as u64));
        }
        assert!(e.words().len() > initial, "generator topped up");
        assert_eq!(e.status(), Status::Running);
        assert!(!e.tick(t0 + Duration::from_secs(14)));
        assert!(e.tick(t0 + Duration::from_secs(15)));
        assert!(e.is_finished());
        assert_eq!(
            e.remaining_at(t0 + Duration::from_secs(99)),
            Some(Duration::ZERO)
        );
        // Input after finish is ignored.
        let n = e.keystrokes().len();
        e.type_char('a');
        assert_eq!(e.keystrokes().len(), n);
    }

    #[test]
    fn space_after_last_word_finishes() {
        let mut e = engine(Mode::Words(2));
        type_str(&mut e, "ab cx ", Instant::now());
        assert!(e.is_finished());
        assert!(!e.words()[1].is_correct());
    }
}
