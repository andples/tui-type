//! State for the `:profile` screen: a list of profiles to switch between,
//! and an editor that builds a profile from any subset of the settings.

use super::{Profile, ProfileRegistry, ProfileSettings, SettingKey, is_name_char};
use crate::command::Completions;
use crate::config::{
    Config, FONT_SIZE_RANGE, Keyboard, LINES_RANGE, Pace, PbEffect, WORDS_PER_LINE_RANGE,
};
use crate::test::mode::Mode;
use crate::ui::widgets::Selection;

pub enum ProfileMenu {
    /// Cursor over the profiles plus one extra "new profile" row at the
    /// end. `confirm_delete` asks before removing the selected profile.
    List {
        selected: Selection,
        confirm_delete: bool,
    },
    Edit(Box<Editor>),
}

impl Default for ProfileMenu {
    fn default() -> Self {
        ProfileMenu::List {
            selected: Selection::wrapping(1),
            confirm_delete: false,
        }
    }
}

impl ProfileMenu {
    /// The list, with `name` highlighted if it exists.
    pub fn list_at(registry: &ProfileRegistry, name: Option<&str>) -> Self {
        let mut selected = Selection::wrapping(registry.len() + 1);
        if let Some(i) = name.and_then(|n| registry.names().position(|p| p == n)) {
            selected.select(i);
        }
        ProfileMenu::List {
            selected,
            confirm_delete: false,
        }
    }

    /// The profile under the cursor in the list; `None` on the "new" row.
    pub fn selected_profile<'a>(&self, registry: &'a ProfileRegistry) -> Option<&'a Profile> {
        match self {
            ProfileMenu::List { selected, .. } => registry.iter().nth(selected.selected),
            ProfileMenu::Edit(_) => None,
        }
    }

    pub fn move_by(&mut self, delta: isize, registry: &ProfileRegistry) {
        match self {
            ProfileMenu::List {
                selected,
                confirm_delete,
            } => {
                *confirm_delete = false;
                selected.set_len(registry.len() + 1);
                selected.move_by(delta);
            }
            ProfileMenu::Edit(e) => e.row = wrap(e.row, delta, Editor::ROWS),
        }
    }
}

/// The item `dir` places after `current` in `list`, wrapping; an unknown
/// `current` (a custom value) steps onto the list's first or last item.
fn step<T: Clone + PartialEq>(list: &[T], current: &T, dir: isize) -> T {
    match list.iter().position(|x| x == current) {
        Some(i) => list[wrap(i, dir, list.len())].clone(),
        None if list.is_empty() => current.clone(),
        None if dir > 0 => list[0].clone(),
        None => list[list.len() - 1].clone(),
    }
}

fn wrap(i: usize, delta: isize, len: usize) -> usize {
    (i as isize + delta).rem_euclid(len as isize) as usize
}

/// Creates or edits one profile. Row 0 is the name; rows 1.. are the
/// settings in `SettingKey::ALL` order, each checked (in the profile) or not.
pub struct Editor {
    /// Name of the profile being edited; `None` for a new one.
    pub original: Option<String>,
    pub name: String,
    pub settings: ProfileSettings,
    pub row: usize,
    /// The profile as it was on disk, so unchecking then re-checking a
    /// setting brings back its saved value rather than the live one.
    base: ProfileSettings,
}

impl Editor {
    pub const ROWS: usize = SettingKey::ALL.len() + 1;

    pub fn new() -> Self {
        Self {
            original: None,
            name: String::new(),
            settings: ProfileSettings::default(),
            row: 0,
            base: ProfileSettings::default(),
        }
    }

    pub fn edit(profile: &Profile) -> Self {
        Self {
            original: Some(profile.name.clone()),
            name: profile.name.clone(),
            settings: profile.settings.clone(),
            // Start on the settings; the name rarely changes.
            row: 1,
            base: profile.settings.clone(),
        }
    }

    pub fn on_name(&self) -> bool {
        self.row == 0
    }

    pub fn key(&self) -> Option<SettingKey> {
        self.row
            .checked_sub(1)
            .and_then(|i| SettingKey::ALL.get(i).copied())
    }

    /// Check or uncheck the focused setting. A newly checked setting takes
    /// its saved value if it had one, else the live value from `config`.
    pub fn toggle(&mut self, config: &Config) {
        let Some(k) = self.key() else { return };
        if self.settings.is_set(k) {
            self.settings.clear(k);
        } else if self.base.is_set(k) {
            self.settings.copy_from(k, &self.base);
        } else {
            self.settings.capture(k, config);
        }
    }

    /// Check the focused setting with its live value from `config`.
    pub fn capture(&mut self, config: &Config) {
        if let Some(k) = self.key() {
            self.settings.capture(k, config);
        }
    }

    /// Check every setting with its live value (a snapshot of everything).
    pub fn capture_all(&mut self, config: &Config) {
        for k in SettingKey::ALL {
            self.settings.capture(*k, config);
        }
    }

    /// Step the focused setting's value (checking it first if needed):
    /// on/off flips, numbers move by one, the rest cycle through their
    /// presets or the installed themes, languages and fonts.
    pub fn cycle(&mut self, dir: isize, config: &Config, comps: &Completions) {
        let Some(k) = self.key() else { return };
        if !self.settings.is_set(k) {
            self.settings.capture(k, config);
        }
        let s = &mut self.settings;
        let flip = |b: &mut Option<bool>| *b = b.map(|v| !v);
        let nudge = |n: &mut Option<u8>, (lo, hi): (u8, u8)| {
            *n = n.map(|v| (v as isize + dir).clamp(lo as isize, hi as isize) as u8);
        };
        match k {
            SettingKey::Mode => {
                let modes: Vec<Mode> = Mode::TIME_PRESETS
                    .iter()
                    .map(|t| Mode::Time(*t))
                    .chain(Mode::WORD_PRESETS.iter().map(|w| Mode::Words(*w)))
                    .collect();
                s.mode = s.mode.map(|m| step(&modes, &m, dir));
            }
            SettingKey::Language => {
                s.language = s.language.take().map(|l| step(&comps.languages, &l, dir));
            }
            SettingKey::Theme => s.theme = s.theme.take().map(|t| step(&comps.themes, &t, dir)),
            SettingKey::Font => {
                let fonts: Vec<String> = std::iter::once(String::new())
                    .chain(comps.fonts.iter().cloned())
                    .collect();
                s.font = s.font.take().map(|f| step(&fonts, &f, dir));
            }
            SettingKey::FontSize => nudge(&mut s.font_size, FONT_SIZE_RANGE),
            SettingKey::WordsPerLine => nudge(&mut s.words_per_line, WORDS_PER_LINE_RANGE),
            SettingKey::Lines => nudge(&mut s.lines, LINES_RANGE),
            SettingKey::Fullscreen => flip(&mut s.fullscreen),
            SettingKey::Punctuation => flip(&mut s.punctuation),
            SettingKey::Numbers => flip(&mut s.numbers),
            SettingKey::Zen => flip(&mut s.zen),
            SettingKey::ResultsChart => flip(&mut s.results.chart),
            SettingKey::ResultsBreakdown => flip(&mut s.results.char_breakdown),
            SettingKey::ResultsConsistency => flip(&mut s.results.consistency),
            SettingKey::ResultsRaw => flip(&mut s.results.raw),
            SettingKey::ResultsKeys => flip(&mut s.results.keys),
            SettingKey::PbEffect => {
                s.pb_effect = s.pb_effect.map(|e| step(&PbEffect::ALL, &e, dir));
            }
            SettingKey::Pace => s.pace = s.pace.map(|p| step(&Pace::PRESETS, &p, dir)),
            SettingKey::Keyboard => {
                s.keyboard = s.keyboard.map(|k| step(&Keyboard::ALL, &k, dir));
            }
        }
    }

    /// The theme to preview while the theme row is focused and checked.
    pub fn preview_theme(&self) -> Option<&str> {
        match self.key() {
            Some(SettingKey::Theme) => self.settings.theme.as_deref(),
            _ => None,
        }
    }

    pub fn insert(&mut self, c: char) {
        if self.on_name() && is_name_char(c) && self.name.len() < 32 {
            self.name.push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.on_name() {
            self.name.pop();
        }
    }

    pub fn profile(&self) -> Profile {
        Profile {
            name: self.name.clone(),
            settings: self.settings.clone(),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::mode::Mode;

    #[test]
    fn list_wraps_over_profiles_plus_new_row() {
        let reg = ProfileRegistry::default();
        let mut m = ProfileMenu::default();
        m.move_by(1, &reg);
        assert!(matches!(&m, ProfileMenu::List { selected, .. } if selected.selected == 0));
        assert!(m.selected_profile(&reg).is_none());
    }

    #[test]
    fn editor_builds_a_subset() {
        let mut config = Config::default();
        let mut e = Editor::new();
        for c in "sprint!".chars() {
            e.insert(c);
        }
        assert_eq!(e.name, "sprint");
        e.row = 1;
        assert_eq!(e.key(), Some(SettingKey::Mode));
        e.insert('x');
        assert_eq!(e.name, "sprint");
        config.mode = Mode::Time(15);
        e.toggle(&config);
        assert_eq!(e.settings.mode, Some(Mode::Time(15)));
        e.toggle(&config);
        assert!(e.settings.is_empty());
        e.capture_all(&config);
        assert_eq!(e.settings.keys(), SettingKey::ALL);
    }

    #[test]
    fn cycling_values_in_place() {
        let comps = Completions {
            themes: vec!["default".into(), "nord".into()],
            ..Default::default()
        };
        let config = Config::default();
        let mut e = Editor::new();
        e.row = 1;
        e.cycle(-1, &config, &comps);
        assert_eq!(e.settings.mode, Some(Mode::Time(15)));
        e.cycle(-1, &config, &comps);
        assert_eq!(e.settings.mode, Some(Mode::Words(100)));
        let at = |k| 1 + SettingKey::ALL.iter().position(|x| *x == k).unwrap();
        e.row = at(SettingKey::Theme);
        e.cycle(1, &config, &comps);
        assert_eq!(e.preview_theme(), Some("nord"));
        e.cycle(1, &config, &comps);
        assert_eq!(e.settings.theme.as_deref(), Some("default"));
        e.row = at(SettingKey::FontSize);
        for _ in 0..20 {
            e.cycle(1, &config, &comps);
        }
        assert_eq!(e.settings.font_size, Some(FONT_SIZE_RANGE.1));
        e.row = at(SettingKey::ResultsRaw);
        e.cycle(1, &config, &comps);
        assert_eq!(e.settings.results.raw, Some(false));
        e.row = at(SettingKey::Font);
        e.cycle(1, &config, &comps);
        assert_eq!(e.settings.font.as_deref(), Some(""));
        assert!(e.settings.validate().is_ok());
    }

    #[test]
    fn retoggling_restores_saved_value() {
        let saved = Profile {
            name: "p".into(),
            settings: ProfileSettings {
                theme: Some("nord".into()),
                ..Default::default()
            },
        };
        let config = Config::default();
        let mut e = Editor::edit(&saved);
        e.row = 1 + SettingKey::ALL
            .iter()
            .position(|k| *k == SettingKey::Theme)
            .unwrap();
        e.toggle(&config);
        assert_eq!(e.settings.theme, None);
        e.toggle(&config);
        assert_eq!(e.settings.theme.as_deref(), Some("nord"));
        e.capture(&config);
        assert_eq!(e.settings.theme.as_deref(), Some("default"));
    }
}
