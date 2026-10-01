//! Keyboard layouts for the results screen's missed-keys heatmap: which
//! key each character is typed on, and what's printed on the keys.

use serde::{Deserialize, Serialize};

/// The layout the missed-keys keyboard is drawn in. Purely visual; it
/// never changes what's typed or how it's scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyboard {
    #[default]
    Qwerty,
    Colemak,
    /// Colemak Mod-DH, ANSI with the angle mod.
    #[serde(alias = "colemak-dh", alias = "colemakdh")]
    ColemakDh,
    Dvorak,
    /// Programmer Dvorak: symbols unshifted on the number row.
    #[serde(alias = "dvorak-programmer", alias = "programmer_dvorak")]
    DvorakProgrammer,
    Workman,
    /// German QWERTZ.
    Qwertz,
    /// French AZERTY.
    Azerty,
}

/// One row of keys: what each key types unshifted, and shifted. Both have
/// the same number of characters.
pub type Row = (&'static str, &'static str);

const QWERTY_NUMBERS: Row = ("1234567890-=", "!@#$%^&*()_+");

impl Keyboard {
    pub const ALL: [Keyboard; 8] = [
        Keyboard::Qwerty,
        Keyboard::Colemak,
        Keyboard::ColemakDh,
        Keyboard::Dvorak,
        Keyboard::DvorakProgrammer,
        Keyboard::Workman,
        Keyboard::Qwertz,
        Keyboard::Azerty,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Keyboard::Qwerty => "qwerty",
            Keyboard::Colemak => "colemak",
            Keyboard::ColemakDh => "colemak_dh",
            Keyboard::Dvorak => "dvorak",
            Keyboard::DvorakProgrammer => "dvorak_programmer",
            Keyboard::Workman => "workman",
            Keyboard::Qwertz => "qwertz",
            Keyboard::Azerty => "azerty",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let norm = s.trim().to_lowercase().replace(['-', ' '], "_");
        let norm = match norm.as_str() {
            "colemakdh" | "colemak_mod_dh" | "dh" => "colemak_dh",
            "programmer_dvorak" | "dvorak_prog" | "dvp" => "dvorak_programmer",
            n => n,
        };
        Self::ALL
            .into_iter()
            .find(|k| k.label() == norm)
            .ok_or_else(|| {
                let names: Vec<&str> = Self::ALL.iter().map(|k| k.label()).collect();
                format!("expected {}, got `{s}`", names.join(", "))
            })
    }

    /// The number row, then the three letter rows, ANSI-style (no ISO
    /// extra keys).
    pub fn rows(self) -> [Row; 4] {
        match self {
            Keyboard::Qwerty => [
                QWERTY_NUMBERS,
                ("qwertyuiop[]", "QWERTYUIOP{}"),
                ("asdfghjkl;'", "ASDFGHJKL:\""),
                ("zxcvbnm,./", "ZXCVBNM<>?"),
            ],
            Keyboard::Colemak => [
                QWERTY_NUMBERS,
                ("qwfpgjluy;[]", "QWFPGJLUY:{}"),
                ("arstdhneio'", "ARSTDHNEIO\""),
                ("zxcvbkm,./", "ZXCVBKM<>?"),
            ],
            Keyboard::ColemakDh => [
                QWERTY_NUMBERS,
                ("qwfpbjluy;[]", "QWFPBJLUY:{}"),
                ("arstgmneio'", "ARSTGMNEIO\""),
                ("xcdvzkh,./", "XCDVZKH<>?"),
            ],
            Keyboard::Dvorak => [
                ("1234567890[]", "!@#$%^&*(){}"),
                ("',.pyfgcrl/=", "\"<>PYFGCRL?+"),
                ("aoeuidhtns-", "AOEUIDHTNS_"),
                (";qjkxbmwvz", ":QJKXBMWVZ"),
            ],
            Keyboard::DvorakProgrammer => [
                ("&[{}(=*)+]!#", "%7531902468`"),
                (";,.pyfgcrl/@", ":<>PYFGCRL?^"),
                ("aoeuidhtns-", "AOEUIDHTNS_"),
                ("'qjkxbmwvz", "\"QJKXBMWVZ"),
            ],
            Keyboard::Workman => [
                QWERTY_NUMBERS,
                ("qdrwbjfup;[]", "QDRWBJFUP:{}"),
                ("ashtgyneoi'", "ASHTGYNEOI\""),
                ("zxmcvkl,./", "ZXMCVKL<>?"),
            ],
            Keyboard::Qwertz => [
                ("1234567890ß´", "!\"§$%&/()=?`"),
                ("qwertzuiopü+", "QWERTZUIOPÜ*"),
                ("asdfghjklöä", "ASDFGHJKLÖÄ"),
                ("yxcvbnm,.-", "YXCVBNM;:_"),
            ],
            Keyboard::Azerty => [
                ("&é\"'(-è_çà)=", "1234567890°+"),
                ("azertyuiop^$", "AZERTYUIOP¨£"),
                ("qsdfghjklmù", "QSDFGHJKLM%"),
                ("wxcvbn,;:!", "WXCVBN?./§"),
            ],
        }
    }

    /// The key `c` is typed on, named by its unshifted character, if it's
    /// on the drawn keyboard.
    pub fn key_for(self, c: char) -> Option<char> {
        let rows = self.rows();
        let unshifted = |c: char| rows.iter().any(|(plain, _)| plain.contains(c)).then_some(c);
        let shifted = |c: char| {
            rows.iter().find_map(|(plain, shift)| {
                shift
                    .chars()
                    .position(|s| s == c)
                    .and_then(|i| plain.chars().nth(i))
            })
        };
        unshifted(c).or_else(|| shifted(c)).or_else(|| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(l), None) if l != c => unshifted(l),
                _ => None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_pair_up() {
        for kb in Keyboard::ALL {
            for (plain, shift) in kb.rows() {
                assert_eq!(
                    plain.chars().count(),
                    shift.chars().count(),
                    "{} row {plain}",
                    kb.label()
                );
            }
            let [_, top, home, bottom] = kb.rows();
            let letters: String = [top.0, home.0, bottom.0].concat();
            for c in 'a'..='z' {
                assert!(letters.contains(c), "{} lacks {c}", kb.label());
            }
        }
    }

    #[test]
    fn every_key_maps_to_itself() {
        for kb in Keyboard::ALL {
            for (plain, _) in kb.rows() {
                for k in plain.chars() {
                    assert_eq!(kb.key_for(k), Some(k), "{}", kb.label());
                }
            }
        }
    }

    #[test]
    fn shifted_characters_find_their_key() {
        assert_eq!(Keyboard::Qwerty.key_for('!'), Some('1'));
        assert_eq!(Keyboard::Qwerty.key_for('A'), Some('a'));
        assert_eq!(Keyboard::Dvorak.key_for('"'), Some('\''));
        assert_eq!(Keyboard::DvorakProgrammer.key_for('7'), Some('['));
        assert_eq!(Keyboard::DvorakProgrammer.key_for('1'), Some('('));
        assert_eq!(Keyboard::Azerty.key_for('1'), Some('&'));
        assert_eq!(Keyboard::Qwertz.key_for('Ü'), Some('ü'));
        assert_eq!(Keyboard::Qwerty.key_for('é'), None);
        assert_eq!(Keyboard::Qwerty.key_for(' '), None);
    }

    #[test]
    fn parse_takes_labels_and_spellings() {
        for kb in Keyboard::ALL {
            assert_eq!(Keyboard::parse(kb.label()), Ok(kb));
        }
        assert_eq!(Keyboard::parse("Colemak-DH"), Ok(Keyboard::ColemakDh));
        assert_eq!(
            Keyboard::parse("programmer dvorak"),
            Ok(Keyboard::DvorakProgrammer)
        );
        assert!(Keyboard::parse("bepo").is_err());
    }
}
