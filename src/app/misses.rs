//! Which keys a finished test went wrong on, for the results screen's
//! keyboard heatmap. Pure: counted from the engine's words (`typed` against
//! `target`), so it never touches scoring or the keylog.

use std::collections::BTreeMap;

use crate::config::Keyboard;
use crate::test::engine::Word;

/// Mistyped target characters, as they were in the words. Which key each
/// one is on depends on the keyboard (`on`), so changing the layout on the
/// results screen redraws the same run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Misses {
    pub chars: BTreeMap<char, u32>,
}

impl Misses {
    /// Count every target character that was typed as something else.
    /// Characters not typed yet (a time test's last word, a skipped word's
    /// tail) and extra characters past the end of a word have no target
    /// and aren't counted.
    pub fn from_words(words: &[Word]) -> Self {
        let mut m = Self::default();
        for w in words {
            for (target, typed) in w.target.iter().zip(&w.typed) {
                if target != typed {
                    *m.chars.entry(*target).or_default() += 1;
                }
            }
        }
        m
    }

    pub fn total(&self) -> u32 {
        self.chars.values().sum()
    }

    /// The misses per key of `kb`.
    pub fn on(&self, kb: Keyboard) -> KeyMisses {
        let mut m = KeyMisses {
            keyboard: kb,
            ..KeyMisses::default()
        };
        for (c, n) in &self.chars {
            match kb.key_for(*c) {
                Some(k) => *m.keys.entry(k).or_default() += n,
                None => m.other += n,
            }
        }
        m
    }
}

/// Mistyped characters per key of one keyboard, keyed by the key's
/// unshifted character (on QWERTY `A` and `a` are both `a`, `!` is `1`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMisses {
    pub keyboard: Keyboard,
    pub keys: BTreeMap<char, u32>,
    /// Misses on characters that aren't on the drawn keyboard (accents,
    /// other scripts, space).
    pub other: u32,
}

impl KeyMisses {
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
        self.keyboard.rows()[0].0.chars().any(|k| self.get(k) > 0)
    }
}

/// Columns one key takes on screen: ` q ` plus a gap.
pub const KEY_COLS: u16 = 4;
/// How far each drawn row is shifted right, in columns, so the rows
/// stagger like a real keyboard (number row first).
const ROW_SHIFT: [u16; 4] = [0, 2, 3, 5];

/// Where each key of `kb` is drawn: (key, column, row) from the
/// keyboard's top left. The number row is only drawn when `numbers` is set; without it the
/// letter rows move up.
pub fn layout(kb: Keyboard, numbers: bool) -> Vec<(char, u16, u16)> {
    let rows = kb
        .rows()
        .into_iter()
        .zip(ROW_SHIFT)
        .map(|((keys, _), shift)| (keys, shift))
        .skip(usize::from(!numbers));
    rows.enumerate()
        .flat_map(|(y, (keys, shift))| {
            keys.chars()
                .enumerate()
                .map(move |(x, k)| (k, shift + x as u16 * KEY_COLS, y as u16))
        })
        .collect()
}

/// Width and height of the drawn keyboard.
pub fn layout_size(kb: Keyboard, numbers: bool) -> (u16, u16) {
    let cells = layout(kb, numbers);
    let w = cells
        .iter()
        .map(|(_, x, _)| x + KEY_COLS - 1)
        .max()
        .unwrap_or(0);
    let h = cells.iter().map(|(_, _, y)| y + 1).max().unwrap_or(0);
    (w, h)
}

impl KeyMisses {
    /// The worst keys, most misses first (alphabetical on a tie).
    pub fn worst(&self, n: usize) -> Vec<(char, u32)> {
        let mut v: Vec<(char, u32)> = self.keys.iter().map(|(k, c)| (*k, *c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
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

    fn qwerty(words: &[Word]) -> KeyMisses {
        Misses::from_words(words).on(Keyboard::Qwerty)
    }

    fn word(target: &str, typed: &str) -> Word {
        Word {
            target: target.chars().collect(),
            typed: typed.chars().collect(),
        }
    }

    #[test]
    fn counts_wrong_target_characters() {
        let m = qwerty(&[
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
        let m = qwerty(&[word("about", "ab"), word("go", "gooo"), word("later", "")]);
        assert_eq!(m, KeyMisses::default());
        assert_eq!(Misses::from_words(&[word("go", "gooo")]).total(), 0);
        assert_eq!(m.total(), 0);
    }

    #[test]
    fn shifted_characters_count_on_their_key() {
        let m = qwerty(&[word("Hello!", "hello1"), word("x?", "x/")]);
        assert_eq!(m.get('h'), 1);
        assert_eq!(m.get('1'), 1);
        assert_eq!(m.get('/'), 1);
        assert!(m.on_number_row());
    }

    #[test]
    fn characters_off_the_keyboard_are_counted_apart() {
        let m = qwerty(&[word("café", "cafe")]);
        assert_eq!(m.other, 1);
        assert!(m.keys.is_empty());
        assert_eq!(m.total(), 1);
    }

    #[test]
    fn repeated_misses_add_up() {
        let m = qwerty(&[word("aaa", "sss"), word("ab", "sb")]);
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
    fn keyboard_layout_staggers_rows() {
        let letters = layout(Keyboard::Qwerty, false);
        assert_eq!(letters.len(), 12 + 11 + 10);
        let at = |cells: &[(char, u16, u16)], k| *cells.iter().find(|c| c.0 == k).unwrap();
        assert_eq!(at(&letters, 'q'), ('q', 2, 0));
        assert_eq!(at(&letters, 'w'), ('w', 6, 0));
        assert_eq!(at(&letters, 'a'), ('a', 3, 1));
        assert_eq!(at(&letters, 'z'), ('z', 5, 2));
        let all = layout(Keyboard::Qwerty, true);
        assert_eq!(at(&all, '1'), ('1', 0, 0));
        assert_eq!(at(&all, 'q'), ('q', 2, 1));
        assert_eq!(layout_size(Keyboard::Qwerty, false), (2 + 11 * 4 + 3, 3));
        assert_eq!(layout_size(Keyboard::Qwerty, true), (2 + 11 * 4 + 3, 4));
    }

    #[test]
    fn worst_keys_first() {
        let m = qwerty(&[word("aab", "ssc"), word("eed", "ffx")]);
        assert_eq!(m.worst(2), vec![('a', 2), ('e', 2)]);
        assert_eq!(m.worst(10).len(), 4);
    }

    #[test]
    fn the_same_run_lands_on_other_layouts() {
        let m = Misses::from_words(&[word("rst;", "rrt:")]);
        let q = m.on(Keyboard::Qwerty);
        assert_eq!((q.get('r'), q.get('s'), q.get(';')), (0, 1, 1));
        let c = m.on(Keyboard::Colemak);
        assert_eq!(c.get('s'), 1);
        assert_eq!(c.get(';'), 1);
        let d = m.on(Keyboard::DvorakProgrammer);
        assert_eq!(d.get(';'), 1, "`:` is shifted `;` there too");
        let at = |kb, k| *layout(kb, false).iter().find(|c| c.0 == k).unwrap();
        assert_eq!(at(Keyboard::Dvorak, 'a'), ('a', 3, 1));
        assert_eq!(at(Keyboard::ColemakDh, 'x'), ('x', 5, 2));
    }

    #[test]
    fn number_row_follows_the_layout() {
        let m = Misses::from_words(&[word("7", "8")]);
        assert!(m.on(Keyboard::Qwerty).on_number_row());
        let dp = m.on(Keyboard::DvorakProgrammer);
        assert_eq!(dp.get('['), 1);
        assert!(dp.on_number_row());
    }
}
