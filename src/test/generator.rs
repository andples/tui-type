//! Word generation. `WordGenerator` is the seam for future learning modes —
//! the engine only ever asks "give me `n` more words".

use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};

pub trait WordGenerator {
    fn next_words(&mut self, n: usize) -> Vec<String>;
}

/// Modifiers applied on top of the plain word list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub punctuation: bool,
    pub numbers: bool,
}

/// Uniformly random words from a list, never the same word twice in a row,
/// with optional monkeytype-style punctuation and numbers.
pub struct RandomGenerator {
    words: Vec<String>,
    mods: Modifiers,
    rng: StdRng,
    last: Option<String>,
    /// Next word starts a sentence (capitalize) when punctuation is on.
    sentence_start: bool,
}

impl RandomGenerator {
    pub fn new(words: Vec<String>, mods: Modifiers) -> Self {
        Self::with_rng(words, mods, StdRng::from_rng(&mut rand::rng()))
    }

    pub fn with_seed(words: Vec<String>, mods: Modifiers, seed: u64) -> Self {
        Self::with_rng(words, mods, StdRng::seed_from_u64(seed))
    }

    fn with_rng(words: Vec<String>, mods: Modifiers, rng: StdRng) -> Self {
        assert!(!words.is_empty(), "word list must not be empty");
        Self {
            words,
            mods,
            rng,
            last: None,
            sentence_start: true,
        }
    }

    fn pick(&mut self) -> String {
        if self.words.len() == 1 {
            return self.words[0].clone();
        }
        loop {
            let w = self.words.choose(&mut self.rng).expect("non-empty");
            if self.last.as_deref() != Some(w.as_str()) {
                return w.clone();
            }
        }
    }

    fn chance(&mut self, p: f64) -> bool {
        self.rng.random_bool(p)
    }

    fn number(&mut self) -> String {
        let digits = self.rng.random_range(1..=4u32);
        let max = 10u32.pow(digits);
        self.rng.random_range(0..max).to_string()
    }

    fn punctuate(&mut self, mut word: String) -> String {
        if self.sentence_start {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                word = first.to_uppercase().chain(chars).collect();
            }
            self.sentence_start = false;
        }
        let roll: f64 = self.rng.random();
        match roll {
            r if r < 0.10 => {
                word.push('.');
                self.sentence_start = true;
            }
            r if r < 0.13 => {
                word.push('?');
                self.sentence_start = true;
            }
            r if r < 0.16 => {
                word.push('!');
                self.sentence_start = true;
            }
            r if r < 0.25 => word.push(','),
            r if r < 0.27 => word.push(';'),
            r if r < 0.29 => word.push(':'),
            r if r < 0.32 => word = format!("\"{word}\""),
            r if r < 0.34 => word = format!("'{word}'"),
            r if r < 0.36 => word = format!("({word})"),
            r if r < 0.38 => {
                let next = self.pick();
                word = format!("{word}-{next}");
            }
            _ => {}
        }
        word
    }
}

impl WordGenerator for RandomGenerator {
    fn next_words(&mut self, n: usize) -> Vec<String> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let mut word = if self.mods.numbers && self.chance(0.1) {
                self.number()
            } else {
                self.pick()
            };
            self.last = Some(word.clone());
            if self.mods.punctuation {
                word = self.punctuate(word);
            }
            out.push(word);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> Vec<String> {
        ["alpha", "beta", "gamma", "delta"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn plain_words_only_from_list() {
        let mut g = RandomGenerator::with_seed(words(), Modifiers::default(), 1);
        let out = g.next_words(200);
        assert_eq!(out.len(), 200);
        let list = words();
        assert!(out.iter().all(|w| list.contains(w)));
        assert!(out.windows(2).all(|w| w[0] != w[1]), "no immediate repeats");
    }

    #[test]
    fn deterministic_with_seed() {
        let a = RandomGenerator::with_seed(words(), Modifiers::default(), 7).next_words(20);
        let b = RandomGenerator::with_seed(words(), Modifiers::default(), 7).next_words(20);
        assert_eq!(a, b);
    }

    #[test]
    fn punctuation_capitalizes_first_and_adds_marks() {
        let mods = Modifiers {
            punctuation: true,
            numbers: false,
        };
        let out = RandomGenerator::with_seed(words(), mods, 3).next_words(300);
        assert!(out[0].chars().next().unwrap().is_uppercase());
        assert!(out.iter().any(|w| w.ends_with('.')));
        assert!(out.iter().any(|w| w.ends_with(',')));
    }

    #[test]
    fn numbers_inserts_digits() {
        let mods = Modifiers {
            punctuation: false,
            numbers: true,
        };
        let out = RandomGenerator::with_seed(words(), mods, 5).next_words(300);
        let nums = out
            .iter()
            .filter(|w| w.chars().all(|c| c.is_ascii_digit()))
            .count();
        assert!(nums > 10 && nums < 100, "got {nums}");
    }
}
