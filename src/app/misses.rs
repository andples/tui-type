//! Which keys a finished test went wrong on, for the results screen's
//! keyboard heatmap. Pure: counted from the engine's words (`typed` against
//! `target`), so it never touches scoring or the keylog.

use std::collections::BTreeMap;

use crate::test::engine::Word;

/// The keyboard drawn on the results screen: the number row, then the three
/// letter rows of QWERTY, each with its indent in key widths ×2.
pub const NUMBER_ROW: &str = "1234567890-=";
pub const LETTER_ROWS: [&str; 3] = ["qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"];

/// Mistyped characters per key, keyed by the unshifted key (`A` and `a`
/// are both `a`, `!` is `1`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMisses {
    pub keys: BTreeMap<char, u32>,
    /// Misses on characters that aren't on the drawn keyboard (accents,
    /// other scripts).
    pub other: u32,
}

impl KeyMisses {
    /// Count every target character that was typed as something else.
    /// Characters not typed yet (a time test's last word, a skipped word's
    /// tail) and extra characters past the end of a word have no target key
    /// and aren't counted.
    pub fn from_words(words: &[Word]) -> Self {
        let mut m = Self::default();
        for w in words {
            for (target, typed) in w.target.iter().zip(&w.typed) {
                if target != typed {
                    match key_for(*target) {
                        Some(k) => *m.keys.entry(k).or_default() += 1,
                        None => m.other += 1,
                    }
                }
            }
        }
        m
    }

    pub fn get(&self, key: char) -> u32 {
        self.keys.get(&key).copied().unwrap_or(0)
    }

    pub fn total(&self) -> u32 {
        self.keys.values().sum::<u32>() + self.other
    }

    /// Most misses on one key.
    pub fn max(&self) -> u32 {
        self.keys.values().copied().max().unwrap_or(0)
    }

    pub fn on_number_row(&self) -> bool {
        NUMBER_ROW.chars().any(|k| self.get(k) > 0)
    }
}

/// The key a character is typed on, if it's on the drawn keyboard.
pub fn key_for(c: char) -> Option<char> {
    let base = match c {
        'A'..='Z' => c.to_ascii_lowercase(),
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        '+' => '=',
        '{' => '[',
        '}' => ']',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        c => c,
    };
    let on_board = NUMBER_ROW.contains(base) || LETTER_ROWS.iter().any(|r| r.contains(base));
    on_board.then_some(base)
}

/// How hot a key is drawn: 0 clean, then 1–3 by its share of the worst
/// key's count (3 is the worst).
pub fn heat(count: u32, max: u32) -> u8 {
    if count == 0 || max == 0 {
        return 0;
    }
    match count * 3 {
        n if n <= max => 1,
        n if n <= max * 2 => 2,
        _ => 3,
    }
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

    #[test]
    fn counts_wrong_target_characters() {
        let m = KeyMisses::from_words(&[
            word("the", "teh"),
            word("quick", "quick"),
            word("brown", "briwn"),
            word("hat", "hag"),
        ]);
        assert_eq!(m.get('h'), 1);
        assert_eq!(m.get('e'), 1);
        assert_eq!(m.get('o'), 1);
        assert_eq!(m.get('t'), 1);
        assert_eq!(m.get('i'), 0, "the key that was hit isn't the miss");
        assert_eq!(m.total(), 4);
        assert_eq!(m.max(), 1);
    }

    #[test]
    fn untyped_and_extra_characters_are_not_misses() {
        let m = KeyMisses::from_words(&[
            word("about", "ab"),
            word("go", "gooo"),
            word("later", ""),
        ]);
        assert_eq!(m, KeyMisses::default());
        assert_eq!(m.total(), 0);
    }

    #[test]
    fn shifted_characters_count_on_their_key() {
        let m = KeyMisses::from_words(&[word("Hello!", "hello1"), word("x?", "x/")]);
        assert_eq!(m.get('h'), 1);
        assert_eq!(m.get('1'), 1);
        assert_eq!(m.get('/'), 1);
        assert!(m.on_number_row());
    }

    #[test]
    fn characters_off_the_keyboard_are_counted_apart() {
        let m = KeyMisses::from_words(&[word("café", "cafe")]);
        assert_eq!(m.other, 1);
        assert!(m.keys.is_empty());
        assert_eq!(m.total(), 1);
        assert_eq!(key_for('é'), None);
        assert_eq!(key_for(' '), None);
    }

    #[test]
    fn repeated_misses_add_up() {
        let m = KeyMisses::from_words(&[word("aaa", "sss"), word("ab", "sb")]);
        assert_eq!(m.get('a'), 4);
        assert_eq!(m.max(), 4);
        assert!(!m.on_number_row());
    }

    #[test]
    fn heat_scales_to_the_worst_key() {
        assert_eq!(heat(0, 0), 0);
        assert_eq!(heat(0, 5), 0);
        assert_eq!(heat(1, 1), 3);
        assert_eq!(heat(1, 6), 1);
        assert_eq!(heat(2, 6), 1);
        assert_eq!(heat(3, 6), 2);
        assert_eq!(heat(4, 6), 2);
        assert_eq!(heat(5, 6), 3);
        assert_eq!(heat(6, 6), 3);
    }

    #[test]
    fn every_drawn_key_maps_to_itself() {
        for k in NUMBER_ROW.chars().chain(LETTER_ROWS.concat().chars()) {
            assert_eq!(key_for(k), Some(k));
        }
    }
}
