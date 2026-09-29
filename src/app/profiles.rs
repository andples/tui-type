//! The profile screen's behaviour: switching profiles on and off, and
//! creating, editing and deleting them.

use super::action::ProfileAction;
use super::input::ProfileInput;
use super::{App, Screen};
use crate::config::Config;
use crate::profile::{Editor, ProfileMenu, SettingKey};

impl App {
    pub(super) fn open_profiles(&mut self) {
        let latest = self.config.profiles.last().cloned();
        self.profile_menu = ProfileMenu::list_at(&self.profiles, latest.as_deref());
        self.push_screen(Screen::Profiles);
    }

    pub(super) fn profile_input(&self) -> ProfileInput {
        match &self.profile_menu {
            ProfileMenu::List {
                confirm_delete: true,
                ..
            } => ProfileInput::ConfirmDelete,
            ProfileMenu::List { .. } => ProfileInput::List,
            ProfileMenu::Edit(e) if e.on_name() => ProfileInput::EditName,
            ProfileMenu::Edit(_) => ProfileInput::EditSettings,
        }
    }

    pub(super) fn profile_action(&mut self, action: ProfileAction) {
        use ProfileAction as P;
        let selected = self
            .profile_menu
            .selected_profile(&self.profiles)
            .map(|p| p.name.clone());
        match action {
            P::Up => self.profile_menu.move_by(-1, &self.profiles),
            P::Down => self.profile_menu.move_by(1, &self.profiles),
            P::Toggle => match selected {
                Some(name) => self.toggle_profile(&name),
                None => self.profile_menu = ProfileMenu::Edit(Box::default()),
            },
            P::New => self.profile_menu = ProfileMenu::Edit(Box::default()),
            P::Edit => {
                if let Some(p) = selected.and_then(|n| self.profiles.get(&n)) {
                    self.profile_menu = ProfileMenu::Edit(Box::new(Editor::edit(p)));
                }
            }
            P::Delete | P::CancelDelete => {
                if let ProfileMenu::List { confirm_delete, .. } = &mut self.profile_menu {
                    *confirm_delete = action == P::Delete && selected.is_some();
                }
            }
            P::ConfirmDelete => {
                if let Some(name) = selected {
                    self.delete_profile(&name);
                }
            }
            P::Check | P::Cycle(_) | P::Capture | P::CaptureAll | P::Insert(_) | P::Backspace => {
                if let ProfileMenu::Edit(e) = &mut self.profile_menu {
                    match action {
                        P::Check => e.toggle(&self.config),
                        P::Cycle(d) => e.cycle(d as isize, &self.config, &self.completions),
                        P::Capture => e.capture(&self.config),
                        P::CaptureAll => e.capture_all(&self.config),
                        P::Insert(c) => e.insert(c),
                        _ => e.backspace(),
                    }
                }
            }
            P::Save => self.save_profile(),
            P::Cancel => {
                if let ProfileMenu::Edit(e) = &self.profile_menu {
                    let back = e.original.clone();
                    self.profile_menu = ProfileMenu::list_at(&self.profiles, back.as_deref());
                }
            }
        }
    }

    /// Deselect an active profile (its settings stay as they are), or
    /// activate an inactive one.
    fn toggle_profile(&mut self, name: &str) {
        if self.config.profiles.iter().any(|n| n == name) {
            self.config.profiles.retain(|n| n != name);
            self.notify(format!("profile {name} off"));
            self.save_config();
        } else {
            self.activate_profile(name);
        }
    }

    /// Apply a profile's settings and mark it active, deselecting every
    /// active profile that sets any of the same settings.
    pub(super) fn activate_profile(&mut self, name: &str) {
        let Some(profile) = self.profiles.get(name) else {
            self.notify(format!("unknown profile `{name}`"));
            return;
        };
        let s = &profile.settings;
        if let Some(t) = s.theme.as_ref().filter(|t| self.themes.get(t).is_none()) {
            self.notify(format!(
                "profile {name}: theme `{t}` isn't installed (:install {t})"
            ));
            return;
        }
        if let Some(l) = s
            .language
            .as_ref()
            .filter(|l| self.languages.get(l).is_none())
        {
            self.notify(format!(
                "profile {name}: language `{l}` isn't installed (:install {l})"
            ));
            return;
        }
        let before = self.config.clone();
        let replaced = match self.profiles.activate(name, &mut self.config) {
            Ok(r) => r,
            Err(e) => {
                self.notify(e);
                return;
            }
        };
        if replaced.is_empty() {
            self.notify(format!("profile {name} on"));
        } else {
            self.notify(format!(
                "profile {name} on · replaced {}",
                replaced.join(", ")
            ));
        }
        self.apply_changes(&before);
        self.save_config();
    }

    /// Side effects of settings that changed since `before`.
    fn apply_changes(&mut self, before: &Config) {
        let changed: Vec<SettingKey> = SettingKey::ALL
            .iter()
            .copied()
            .filter(|k| k.differs(before, &self.config))
            .collect();
        if changed.iter().any(|k| k.affects_graphics())
            && let Err(e) = self.apply_graphics()
        {
            self.notify_more(e);
        }
        if changed.iter().any(|k| k.restarts_test()) {
            self.rebuild_test();
        }
    }

    fn save_profile(&mut self) {
        let ProfileMenu::Edit(e) = &self.profile_menu else {
            return;
        };
        let profile = e.profile();
        let original = e.original.clone();
        if let Err(err) = self.profiles.save(profile.clone(), original.as_deref()) {
            self.notify(err);
            return;
        }
        let name = profile.name;
        self.refresh_profile_completions();
        self.profile_menu = ProfileMenu::list_at(&self.profiles, Some(&name));

        // An active profile stays active under its new name and settings.
        let was_active = original
            .as_ref()
            .is_some_and(|o| self.config.profiles.contains(o));
        if was_active {
            for n in &mut self.config.profiles {
                if Some(&*n) == original.as_ref() {
                    *n = name.clone();
                }
            }
            self.activate_profile(&name);
        } else {
            self.notify(format!("profile {name} saved"));
        }
    }

    fn delete_profile(&mut self, name: &str) {
        if let Err(e) = self.profiles.delete(name) {
            self.notify(e);
            return;
        }
        self.config.profiles.retain(|n| n != name);
        self.refresh_profile_completions();
        if let ProfileMenu::List {
            selected,
            confirm_delete,
        } = &mut self.profile_menu
        {
            *confirm_delete = false;
            // Stay on a profile rather than landing on the "new" row.
            let len = self.profiles.len();
            selected.set_len(len + 1);
            selected.select(selected.selected.min(len.saturating_sub(1)));
        }
        self.notify(format!("profile {name} deleted"));
        self.save_config();
    }

    fn refresh_profile_completions(&mut self) {
        self.completions.profiles = self.profiles.names().map(str::to_string).collect();
    }
}
