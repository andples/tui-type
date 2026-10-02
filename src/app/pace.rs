//! The pace caret (`pace` setting): a ghost caret that moves through the
//! words at a steady target speed, so you can see whether you're ahead of
//! your best, your last run or a fixed wpm. Purely visual: it reads the
//! words' lengths and the clock, never the keylog or the scoring.
//!
//! At `wpm` the ghost covers `wpm × 5 / 60` characters a second, counting
//! the space after each word, which is how wpm itself is counted.

use std::time::Duration;

use crate::config::Pace;
use crate::stats::{TestRecord, personal_best};
use crate::test::Mode;

/// Characters per second at `wpm`.
fn chars_per_sec(wpm: f64) -> f64 {
    wpm * 5.0 / 60.0
}

/// Characters the ghost has covered after `elapsed`.
fn chars_at(elapsed: Duration, wpm: f64) -> u64 {
    (elapsed.as_secs_f64() * chars_per_sec(wpm)).floor() as u64
}

/// Where the ghost is after `elapsed`: (word index, character offset), with
/// the offset equal to the word's length while it sits on the space after
/// it. Past the last word it waits at the end of it. `None` without a speed
/// or words.
pub fn position(lengths: &[usize], elapsed: Duration, wpm: f64) -> Option<(usize, usize)> {
    if wpm <= 0.0 || lengths.is_empty() {
        return None;
    }
    let mut left = chars_at(elapsed, wpm);
    for (i, &len) in lengths.iter().enumerate() {
        let span = len as u64 + 1;
        if left < span {
            return Some((i, left as usize));
        }
        left -= span;
    }
    let last = lengths.len() - 1;
    Some((last, lengths[last]))
}

/// How long after `elapsed` the ghost next moves a character, so the event
/// loop wakes only as often as it has to.
pub fn next_move_in(elapsed: Duration, wpm: f64) -> Option<Duration> {
    if wpm <= 0.0 {
        return None;
    }
    let next = (chars_at(elapsed, wpm) + 1) as f64 / chars_per_sec(wpm);
    // A hair past the boundary, so rounding never wakes it a frame early.
    let wait = (next - elapsed.as_secs_f64()).max(0.0) + 0.0005;
    Some(Duration::from_secs_f64(wait))
}

/// The speed a `pace` setting means for `mode` in `language`: a fixed wpm,
/// or the personal best or last result from the history (`None` when there
/// isn't one yet, or the setting is off).
pub fn target_wpm(pace: Pace, records: &[TestRecord], mode: Mode, language: &str) -> Option<f64> {
    match pace {
        Pace::Off => None,
        Pace::Wpm(n) => Some(n as f64),
        Pace::Pb => personal_best(records, mode, language),
        Pace::Last => records
            .iter()
            .rev()
            .find(|r| r.counts() && r.mode == mode && r.language == language)
            .map(|r| r.wpm),
    }
    .filter(|w| *w > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::CharRecord;
    use chrono::Utc;

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    #[test]
    fn walks_through_words_and_spaces() {
        // 60 wpm = 5 characters a second.
        let words = [3, 2, 4]; // "the", "of", "time"
        assert_eq!(position(&words, secs(0.0), 60.0), Some((0, 0)));
        assert_eq!(position(&words, secs(0.21), 60.0), Some((0, 1)));
        // 3 characters in: on the space after "the".
        assert_eq!(position(&words, secs(0.6), 60.0), Some((0, 3)));
        // 4 in: the start of "of".
        assert_eq!(position(&words, secs(0.8), 60.0), Some((1, 0)));
        assert_eq!(position(&words, secs(1.4), 60.0), Some((2, 0)));
        assert_eq!(position(&words, secs(2.0), 60.0), Some((2, 3)));
        // Past the end it waits after the last word.
        assert_eq!(position(&words, secs(9.0), 60.0), Some((2, 4)));
    }

    #[test]
    fn faster_pace_is_further_along() {
        let words = [5; 50];
        let at = |wpm| position(&words, secs(3.0), wpm).unwrap();
        // 3 s at 120 wpm is 30 characters: five words of five plus spaces.
        assert_eq!(at(120.0), (5, 0));
        assert_eq!(at(60.0), (2, 3));
        assert!(at(87.0) > at(60.0) && at(87.0) < at(120.0));
    }

    #[test]
    fn no_speed_no_ghost() {
        assert_eq!(position(&[3, 4], secs(1.0), 0.0), None);
        assert_eq!(position(&[], secs(1.0), 80.0), None);
        assert_eq!(next_move_in(secs(1.0), 0.0), None);
    }

    #[test]
    fn wakes_when_the_ghost_moves() {
        // 60 wpm: a character every 0.2 s.
        let d = next_move_in(secs(0.0), 60.0).unwrap();
        assert!((d.as_secs_f64() - 0.2).abs() < 1e-3, "{d:?}");
        let d = next_move_in(secs(0.35), 60.0).unwrap();
        assert!((d.as_secs_f64() - 0.05).abs() < 1e-3, "{d:?}");
        // Waking where it says moves the ghost on.
        let words = [5; 20];
        let mut t = secs(0.0);
        for _ in 0..30 {
            let before = position(&words, t, 87.0);
            t += next_move_in(t, 87.0).unwrap();
            assert_ne!(position(&words, t, 87.0), before);
        }
    }

    fn rec(mode: Mode, language: &str, wpm: f64) -> TestRecord {
        TestRecord {
            schema: 2,
            ts: Utc::now(),
            mode,
            language: language.into(),
            punctuation: false,
            numbers: false,
            wpm,
            raw: wpm,
            acc: 100.0,
            consistency: 80.0,
            chars: CharRecord {
                correct: 1,
                incorrect: 0,
                extra: 0,
                missed: 0,
            },
            duration_s: 30.0,
            daily_id: None,
            invalid: None,
            missed_chars: Default::default(),
            typed_chars: Default::default(),
        }
    }

    #[test]
    fn targets_come_from_the_setting_and_history() {
        let t30 = Mode::Time(30);
        let rs = [
            rec(t30, "english", 70.0),
            rec(t30, "english", 90.0),
            rec(Mode::Time(15), "english", 120.0),
            rec(t30, "spanish", 50.0),
            rec(t30, "english", 80.0),
            rec(Mode::Words(25), "english", 99.0),
        ];
        assert_eq!(target_wpm(Pace::Off, &rs, t30, "english"), None);
        assert_eq!(target_wpm(Pace::Wpm(87), &rs, t30, "english"), Some(87.0));
        assert_eq!(target_wpm(Pace::Pb, &rs, t30, "english"), Some(90.0));
        assert_eq!(target_wpm(Pace::Last, &rs, t30, "english"), Some(80.0));
        assert_eq!(target_wpm(Pace::Last, &rs, t30, "spanish"), Some(50.0));
        // Nothing to race yet.
        assert_eq!(target_wpm(Pace::Pb, &rs, Mode::Time(60), "english"), None);
        assert_eq!(target_wpm(Pace::Last, &[], t30, "english"), None);
    }
}
