//! Custom word sets: the custom page (`:custom`, or tab past themes on the
//! install screen) lists your sets and the server's shared ones, most
//! installed first; the editor makes and changes sets; `:custom <name>`
//! types one instead of the language and `:custom off` goes back.

use ttyp_core::api::{CustomList, CustomPublish, CustomSet as SharedSet, CustomSummary};

use super::action::{CustomAction, EditAction};
use super::input::CustomInput;
use super::{App, Screen};
use crate::custom::menu::{CustomEditor, EditStep, PAGE, notes};
use crate::custom::{CustomSet, menu::Confirm, valid_name};
use crate::online::{OnlineError, Request};

impl App {
    /// The set being typed instead of the language, if any (and it exists).
    pub fn active_custom(&self) -> Option<&CustomSet> {
        self.customs
            .get(&self.config.custom)
            .filter(|s| !s.words.is_empty())
    }

    /// Keys on the custom page and in the editor depend on this.
    pub(super) fn custom_input(&self) -> CustomInput {
        match self.screen {
            Screen::Custom if self.custom_menu.confirm.is_some() => CustomInput::Confirm,
            Screen::Custom if self.custom_menu.editing => CustomInput::Searching,
            Screen::CustomEdit => match &self.custom_editor {
                Some(e) if e.pending.is_some() => CustomInput::Confirm,
                Some(e) if e.focus_list => CustomInput::Words,
                _ => CustomInput::Typing,
            },
            _ => CustomInput::List,
        }
    }

    pub(super) fn open_custom(&mut self) {
        self.custom_menu.confirm = None;
        self.custom_menu.editing = false;
        self.custom_menu.sync(&self.customs);
        self.search_custom();
        // Like the install screen, esc goes back to what was under it.
        self.push_screen(Screen::Custom);
    }

    /// Fetch the first page of shared sets for the search.
    fn search_custom(&mut self) {
        let menu = &mut self.custom_menu;
        let Some(online) = &mut self.online else {
            menu.loading = false;
            menu.error = Some("offline: shared sets need a server".into());
            return;
        };
        menu.searching();
        online.request(Request::CustomList {
            query: menu.query.clone(),
            offset: 0,
            limit: PAGE,
        });
    }

    fn fetch_more_custom(&mut self) {
        let (menu, Some(online)) = (&mut self.custom_menu, &mut self.online) else {
            return;
        };
        if let Some(offset) = menu.next_page() {
            menu.loading = true;
            online.request(Request::CustomList {
                query: menu.query.clone(),
                offset,
                limit: PAGE,
            });
        }
    }

    pub(super) fn custom_action(&mut self, action: CustomAction) {
        use CustomAction as C;
        let selected = self.custom_menu.selected(&self.customs);
        let menu = &mut self.custom_menu;
        match action {
            C::Up => menu.selection.move_by(-1),
            C::Down => menu.selection.move_by(1),
            C::Top => menu.selection.home(),
            C::Bottom => menu.selection.end(),
            C::Search => menu.editing = true,
            C::StopSearch => menu.editing = false,
            C::SearchChar(c) => {
                menu.query.push(c);
                self.search_custom();
            }
            C::SearchBackspace => {
                if menu.query.pop().is_some() {
                    self.search_custom();
                }
            }
            C::Use => {
                if let Some(e) = selected {
                    self.use_custom(&e.name);
                }
            }
            C::New => {
                self.custom_editor = Some(CustomEditor::new_set());
                self.screen = Screen::CustomEdit;
            }
            C::Edit => match selected.and_then(|e| self.customs.get(&e.name).cloned()) {
                Some(set) => {
                    self.custom_editor = Some(CustomEditor::edit(set));
                    self.screen = Screen::CustomEdit;
                }
                None => self.notify("install it first (enter), then edit your copy"),
            },
            C::Remove => match selected {
                Some(e) if e.local => menu.confirm = Some(Confirm::Remove(e.name)),
                Some(_) => self.notify("not on this machine"),
                None => {}
            },
            C::Publish => match selected.and_then(|e| self.customs.get(&e.name)) {
                Some(set) if set.is_mine() => {
                    let body = CustomPublish {
                        name: set.name.clone(),
                        words: set.words.clone(),
                    };
                    self.with_login(|o| o.request(Request::CustomPublish(body)));
                }
                Some(_) => self.notify("only sets you made can be published"),
                None => self.notify("make a set first (n)"),
            },
            C::Unpublish => match selected {
                Some(e) if e.local && e.author.is_none() && e.installs.is_some() => {
                    menu.confirm = Some(Confirm::Unpublish(e.name));
                }
                Some(_) => self.notify("only your published sets can be taken down"),
                None => {}
            },
            C::Yes => match menu.confirm.take() {
                Some(Confirm::Remove(name)) => self.remove_custom(&name),
                Some(Confirm::Unpublish(name)) => {
                    self.with_login(|o| o.request(Request::CustomUnpublish(name)));
                }
                None => {}
            },
            C::No => menu.confirm = None,
            C::SwitchTab => {
                self.catalog_menu.tab = crate::catalog::Kind::Language;
                self.open_catalog();
                return;
            }
            C::Close => {
                self.dispatch(super::Action::Back);
                return;
            }
        }
        self.custom_menu.sync(&self.customs);
        self.fetch_more_custom();
    }

    /// Run `f` with the online state when logged in; say why not otherwise.
    fn with_login(&mut self, f: impl FnOnce(&mut crate::online::Online)) {
        match &mut self.online {
            None => self.notify("offline: set server in config"),
            Some(o) if !o.logged_in() => self.notify("not logged in (:login)"),
            Some(o) => f(o),
        }
    }

    /// `:custom <name>` or enter: type that set, installing it first when
    /// it's only on the server.
    pub(super) fn use_custom(&mut self, name: &str) {
        if self.customs.contains(name) {
            self.set_custom(name);
            return;
        }
        if !valid_name(name) {
            self.notify(format!("no custom set called `{name}`"));
            return;
        }
        match &mut self.online {
            Some(o) => {
                o.request(Request::CustomInstall(name.to_string()));
                self.notify(format!("installing {name}…"));
            }
            None => self.notify(format!("no custom set called `{name}` (offline)")),
        }
    }

    /// Switch the words to `name` (or back to the language with `""`).
    pub(super) fn set_custom(&mut self, name: &str) {
        if self.refuse_if_daily_locked() {
            return;
        }
        self.config.custom = name.to_string();
        let msg = match self.active_custom() {
            Some(s) => format!(
                "typing custom set {} ({} words) · :custom off for {}",
                s.name,
                s.words.len(),
                self.config.language
            ),
            None if name.is_empty() => format!("back to {}", self.config.language),
            None => format!("{name} has no words yet: add some (e on the custom page)"),
        };
        self.save_config();
        self.rebuild_test();
        self.refresh_custom_names();
        self.notify(msg);
    }

    fn remove_custom(&mut self, name: &str) {
        if let Err(e) = self.customs.remove(name) {
            self.notify(format!("could not remove {name}: {e}"));
            return;
        }
        if self.config.custom == name {
            self.config.custom.clear();
            self.save_config();
            self.rebuild_test();
        }
        self.refresh_custom_names();
        self.notify(format!("removed {name}"));
    }

    /// The palette's `:custom` suggestions.
    pub(super) fn refresh_custom_names(&mut self) {
        self.completions.customs = std::iter::once("off".to_string())
            .chain(self.customs.names().map(str::to_string))
            .collect();
    }

    pub(super) fn custom_edit_action(&mut self, action: EditAction) {
        use EditAction as E;
        let Some(ed) = &mut self.custom_editor else {
            return;
        };
        match action {
            E::Char(c) => {
                ed.input.push(c);
                ed.message = None;
            }
            E::Backspace => {
                ed.input.pop();
            }
            E::DeleteWord => {
                let keep = ed.input.trim_end().rfind(' ').map_or(0, |i| i + 1);
                ed.input.truncate(keep);
            }
            E::Enter => match ed.step {
                EditStep::Name => {
                    if let Err(e) = ed.take_name(&self.customs) {
                        ed.message = Some(e);
                    } else {
                        ed.message = Some("now type words separated by spaces, then enter".into());
                    }
                }
                EditStep::Words => ed.propose(),
            },
            E::Yes => {
                if ed.confirm() > 0 {
                    self.save_editor();
                }
            }
            E::No => ed.cancel(),
            E::ToggleFocus => {
                if ed.step == EditStep::Words && !ed.set.words.is_empty() {
                    ed.focus_list = !ed.focus_list;
                }
            }
            E::Up => ed.selection.move_by(-1),
            E::Down => ed.selection.move_by(1),
            E::Remove => {
                if ed.remove_selected().is_some() {
                    if ed.set.words.is_empty() {
                        ed.focus_list = false;
                    }
                    self.save_editor();
                }
            }
            E::Escape => {
                if !ed.focus_list && !ed.input.is_empty() {
                    ed.input.clear();
                    ed.message = None;
                } else {
                    self.close_editor();
                }
            }
        }
    }

    /// Write the edited set to disk (every change is saved at once).
    fn save_editor(&mut self) {
        let Some(ed) = &mut self.custom_editor else {
            return;
        };
        if let Err(e) = self.customs.save(ed.set.clone()) {
            ed.message = Some(format!("could not save: {e}"));
            return;
        }
        // The set being typed changed: new words for the test underneath.
        if self.config.custom == ed.set.name {
            self.rebuild_test();
        }
        self.refresh_custom_names();
    }

    fn close_editor(&mut self) {
        let Some(ed) = self.custom_editor.take() else {
            return;
        };
        self.screen = Screen::Custom;
        self.custom_menu.sync(&self.customs);
        if ed.step == EditStep::Words && !ed.set.words.is_empty() {
            let mine = ed.set.is_mine();
            self.notify(format!(
                "saved {} · {} words · enter types it{}",
                ed.set.name,
                ed.set.words.len(),
                if mine { ", p publishes it" } else { "" }
            ));
            // Point at it.
            if let Some(i) = self
                .custom_menu
                .entries(&self.customs)
                .iter()
                .position(|e| e.name == ed.set.name)
            {
                self.custom_menu.selection.select(i);
            }
        }
    }

    pub(super) fn custom_list_reply(
        &mut self,
        query: &str,
        result: Result<CustomList, OnlineError>,
    ) {
        match result {
            Ok(list) => self.custom_menu.apply(query, list, &self.customs),
            Err(e) => self.custom_menu.fail(query, e.to_string()),
        }
    }

    pub(super) fn custom_installed(&mut self, name: &str, result: Result<SharedSet, OnlineError>) {
        let set = match result {
            Ok(s) => s,
            Err(OnlineError::Server { status: 404, .. }) => {
                self.notify(format!("no shared set called {name}"));
                return;
            }
            Err(e) => {
                self.notify(format!("install {name}: {e}"));
                return;
            }
        };
        if let Err(e) = crate::custom::check(&set.name, &set.words) {
            self.notify(format!("{name} can't be used: {e}"));
            return;
        }
        let local = CustomSet {
            name: set.name.clone(),
            author: Some(set.author),
            words: set.words,
        };
        if let Err(e) = self.customs.save(local) {
            self.notify(format!("could not save {name}: {e}"));
            return;
        }
        self.custom_menu.sync(&self.customs);
        self.set_custom(&set.name);
        // The install counts changed.
        self.search_custom();
    }

    pub(super) fn custom_published(
        &mut self,
        name: &str,
        result: Result<CustomSummary, OnlineError>,
    ) {
        match result {
            Ok(s) => {
                self.notify(format!(
                    "published {} ({} words) · others can install it from :custom",
                    s.name, s.words
                ));
                self.search_custom();
            }
            Err(e) => self.notify(format!("publish {name}: {e}")),
        }
    }

    pub(super) fn custom_unpublished(&mut self, name: &str, result: Result<(), OnlineError>) {
        match result {
            Ok(()) => {
                self.notify(format!("{name} is off the server · your copy stays"));
                self.search_custom();
            }
            Err(e) => self.notify(format!("unpublish {name}: {e}")),
        }
    }

    /// How the words line reads in the editor's confirmation.
    pub fn pending_summary(ed: &CustomEditor) -> Option<String> {
        let a = ed.pending.as_ref()?;
        const SHOW: usize = 8;
        let mut words = a
            .new
            .iter()
            .take(SHOW)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if a.new.len() > SHOW {
            words.push_str(&format!(" … +{}", a.new.len() - SHOW));
        }
        Some(format!(
            "add {} {} to {}: {words}?{}",
            a.new.len(),
            if a.new.len() == 1 { "word" } else { "words" },
            ed.set.name,
            notes(a)
        ))
    }
}
