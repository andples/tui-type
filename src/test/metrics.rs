//! Result metrics, following monkeytype's definitions:
//!
//! - net wpm  = (chars of correct words incl. their space + correct prefix of
//!   the current word) / 5 / minutes
//! - raw wpm  = all logged keystrokes / 5 / minutes
//! - accuracy = correct keystrokes / all keystrokes
//! - consistency = kogasa(coefficient of variation of per-second raw wpm)

use std::time::Duration;

use super::engine::{Keystroke, TestEngine, Word};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharCounts {
    pub correct: usize,
    pub incorrect: usize,
    pub extra: usize,
    pub missed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub wpm: f64,
    pub raw: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub chars: CharCounts,
    pub duration: Duration,
    /// Raw wpm for each whole second of the test (burst speed).
    pub raw_per_second: Vec<f64>,
    /// Cumulative net wpm at the end of each second.
    pub wpm_per_second: Vec<f64>,
    /// Errors in each second.
    pub errors_per_second: Vec<u32>,
}

impl Metrics {
    pub fn from_engine(engine: &TestEngine) -> Self {
        Self::compute(
            engine.words(),
            engine.current_index(),
            engine.keystrokes(),
            engine.elapsed(),
        )
    }

    pub fn compute(
        words: &[Word],
        current: usize,
        keystrokes: &[Keystroke],
        duration: Duration,
    ) -> Self {
        let minutes = duration.as_secs_f64() / 60.0;
        let total_keys = keystrokes.len();
        let correct_keys = keystrokes.iter().filter(|k| k.correct).count();

        let mut correct_chars = 0usize;
        for (i, w) in words.iter().enumerate() {
            if i < current {
                if w.is_correct() {
                    correct_chars += w.target.len() + 1;
                }
            } else if i == current {
                correct_chars += w
                    .typed
                    .iter()
                    .zip(&w.target)
                    .take_while(|(t, e)| t == e)
                    .count();
            }
        }

        let per_min = |chars: usize| {
            if minutes <= 0.0 {
                0.0
            } else {
                chars as f64 / 5.0 / minutes
            }
        };

        let secs = duration.as_secs_f64().ceil().max(1.0) as usize;
        let mut raw_per_second = vec![0.0; secs];
        let mut errors_per_second = vec![0u32; secs];
        let mut wpm_per_second = vec![0.0; secs];
        let mut keys_per_second = vec![0usize; secs];
        let mut cumulative_correct = 0usize;
        let mut idx = 0;
        for s in 0..secs {
            let end = Duration::from_secs(s as u64 + 1);
            while idx < keystrokes.len() && keystrokes[idx].at < end {
                keys_per_second[s] += 1;
                if keystrokes[idx].correct {
                    cumulative_correct += 1;
                } else {
                    errors_per_second[s] += 1;
                }
                idx += 1;
            }
            raw_per_second[s] = keys_per_second[s] as f64 / 5.0 * 60.0;
            let elapsed_min = ((s + 1) as f64).min(duration.as_secs_f64().max(0.001)) / 60.0;
            wpm_per_second[s] = cumulative_correct as f64 / 5.0 / elapsed_min;
        }

        Self {
            wpm: per_min(correct_chars),
            raw: per_min(total_keys),
            accuracy: if total_keys == 0 {
                0.0
            } else {
                correct_keys as f64 / total_keys as f64 * 100.0
            },
            consistency: consistency(&raw_per_second),
            chars: char_counts(words, current),
            duration,
            raw_per_second,
            wpm_per_second,
            errors_per_second,
        }
    }
}

pub fn char_counts(words: &[Word], current: usize) -> CharCounts {
    let mut c = CharCounts::default();
    for (i, w) in words.iter().enumerate() {
        if w.typed.is_empty() && i > current {
            break;
        }
        for (j, t) in w.typed.iter().enumerate() {
            match w.target.get(j) {
                Some(e) if e == t => c.correct += 1,
                Some(_) => c.incorrect += 1,
                None => c.extra += 1,
            }
        }
        if i < current && w.typed.len() < w.target.len() {
            c.missed += w.target.len() - w.typed.len();
        }
    }
    c
}

/// monkeytype's consistency: 100 * (1 - tanh(cv + cv³/3 + cv⁵/5)), where cv is
/// the coefficient of variation of the per-second raw speeds.
pub fn consistency(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 100.0;
    }
    let mean = samples.iter().sum::<f64>() / n as f64;
    if mean == 0.0 {
        return 0.0;
    }
    let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    let cv = var.sqrt() / mean;
    kogasa(cv)
}

fn kogasa(x: f64) -> f64 {
    100.0 * (1.0 - (x + x.powi(3) / 3.0 + x.powi(5) / 5.0).tanh())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(target: &str, typed: &str) -> Word {
        Word {
            target: target.chars().collect(),
            typed: typed.chars().collect(),
        }
    }

    fn ks(n: usize, correct: bool, secs: f64) -> Vec<Keystroke> {
        (0..n)
            .map(|i| Keystroke {
                at: Duration::from_secs_f64(secs * i as f64 / n as f64),
                correct,
            })
            .collect()
    }

    #[test]
    fn perfect_test() {
        // "hello world" in 6 seconds: 11 chars + 1 space = 12 correct chars.
        let words = vec![word("hello", "hello"), word("world", "world")];
        let keys = ks(11, true, 6.0);
        let m = Metrics::compute(&words, 1, &keys, Duration::from_secs(6));
        // correct chars: "hello"+space = 6, plus "world" prefix 5 = 11 → 11/5/0.1min = 22
        assert!((m.wpm - 22.0).abs() < 1e-9, "{}", m.wpm);
        assert!((m.raw - 22.0).abs() < 1e-9);
        assert_eq!(m.accuracy, 100.0);
        assert_eq!(m.chars.correct, 10);
        assert_eq!(m.raw_per_second.len(), 6);
    }

    #[test]
    fn errors_reduce_accuracy_and_wpm() {
        let words = vec![word("ab", "ax"), word("cd", "cd")];
        let mut keys = ks(2, true, 1.0);
        keys[1].correct = false;
        keys.extend(ks(2, true, 1.0).into_iter().map(|mut k| {
            k.at += Duration::from_secs(1);
            k
        }));
        let m = Metrics::compute(&words, 1, &keys, Duration::from_secs(2));
        assert_eq!(m.accuracy, 75.0);
        // Word 0 wrong → 0; word 1 (current) correct prefix 2 → 2/5/(2/60) = 12
        assert!((m.wpm - 12.0).abs() < 1e-9);
        assert_eq!(m.chars.incorrect, 1);
        assert_eq!(m.chars.correct, 3);
    }

    #[test]
    fn counts_extra_and_missed() {
        let words = vec![word("abc", "a"), word("de", "dezz"), word("fg", "")];
        let c = char_counts(&words, 1);
        assert_eq!(c.missed, 2);
        assert_eq!(c.extra, 2);
        assert_eq!(c.correct, 3);
    }

    #[test]
    fn consistency_bounds() {
        assert_eq!(consistency(&[60.0, 60.0, 60.0]), 100.0);
        let low = consistency(&[0.0, 120.0, 0.0, 120.0]);
        assert!(low < 30.0, "{low}");
        assert_eq!(consistency(&[0.0, 0.0]), 0.0);
    }

    #[test]
    fn zero_duration_is_safe() {
        let m = Metrics::compute(&[word("a", "")], 0, &[], Duration::ZERO);
        assert_eq!(m.wpm, 0.0);
        assert_eq!(m.accuracy, 0.0);
        assert_eq!(m.raw_per_second.len(), 1);
    }
}
