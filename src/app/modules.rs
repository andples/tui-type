//! The modules screen's behaviour: choosing which of a language's modules
//! to download (from `:install`) and which to mix into its tests (when
//! switching to it, or `:modules`).

use super::action::ModuleAction;
use super::{App, Screen};
use crate::catalog::module_menu::{ModuleMenu, ModuleRow, Purpose};
use crate::catalog::{self, Kind, ModuleRegistry, modules};
use crate::test::Status;

impl App {
    /// What the current test's runs are recorded as: the language plus its
    /// mixed-in modules (`code_python+numpy`).
    pub fn language_key(&self) -> String {
        if let Some(set) = self.active_custom() {
            return format!("custom:{}", set.name);
        }
        let lang = self.languages.get_or_default(&self.config.language);
        modules::language_key(&lang.name, &self.selected_modules(&lang.name))
    }

    fn selected_modules(&self, language: &str) -> Vec<&crate::language::Language> {
        let wanted = self
            .config
            .modules
            .get(language)
            .map_or(&[][..], Vec::as_slice);
        self.modules.selected(language, wanted)
    }

    /// `:install` → enter on a language that offers modules: tick the ones
    /// to have.
    pub(super) fn open_module_download(&mut self, language: &str) {
        let Some(entry) = self
            .catalog_menu
            .index()
            .and_then(|i| i.language(language))
            .cloned()
        else {
            return;
        };
        let mut rows: Vec<ModuleRow> = entry
            .modules
            .iter()
            .map(|m| {
                let installed = self.modules.get(language, &m.name).is_some();
                ModuleRow {
                    name: m.name.clone(),
                    display: m.display.clone(),
                    words: m.words,
                    installed,
                    checked: installed,
                }
            })
            .collect();
        // Modules installed by hand stay listed so they can be removed.
        for m in self.modules.of(language) {
            if !rows.iter().any(|r| r.name == m.name) {
                rows.push(ModuleRow {
                    name: m.name.clone(),
                    display: m.display.clone(),
                    words: m.words.len(),
                    installed: true,
                    checked: true,
                });
            }
        }
        self.module_menu = ModuleMenu::new(
            language.to_string(),
            Purpose::Download,
            rows,
            Screen::Catalog,
        );
        self.screen = Screen::Modules;
    }

    /// Switching to a language with modules installed (or `:modules`):
    /// tick the ones to mix in.
    pub(super) fn open_module_pick(&mut self, language: &str) {
        let wanted = self
            .config
            .modules
            .get(language)
            .cloned()
            .unwrap_or_default();
        let rows = self
            .modules
            .of(language)
            .iter()
            .map(|m| ModuleRow {
                name: m.name.clone(),
                display: m.display.clone(),
                words: m.words.len(),
                installed: true,
                checked: wanted.contains(&m.name),
            })
            .collect();
        let back = match self.screen {
            Screen::Modules => self.module_menu.back,
            s => s,
        };
        self.module_menu = ModuleMenu::new(language.to_string(), Purpose::Pick, rows, back);
        self.screen = Screen::Modules;
    }

    /// `:modules`: the current language's checklist.
    pub(super) fn show_modules(&mut self) {
        let language = self.config.language.clone();
        if !self.modules.of(&language).is_empty() {
            self.open_module_pick(&language);
            return;
        }
        let offered = self
            .catalog_menu
            .index()
            .and_then(|i| i.language(&language))
            .is_some_and(|l| !l.modules.is_empty());
        if offered {
            self.open_module_download(&language);
        } else {
            self.notify(format!("{language} has no modules installed (:install)"));
        }
    }

    pub(super) fn module_action(&mut self, action: ModuleAction) {
        use ModuleAction as M;
        let menu = &mut self.module_menu;
        match action {
            M::Move(d) => menu.selection.move_by(d),
            M::Top => menu.selection.home(),
            M::Bottom => menu.selection.end(),
            M::Toggle => menu.toggle(),
            M::ToggleAll => menu.toggle_all(),
            M::Cancel => self.screen = menu.back,
            M::Apply => {
                let back = menu.back;
                match menu.purpose {
                    Purpose::Pick => self.apply_pick(),
                    Purpose::Download => self.apply_download(),
                }
                self.screen = back;
            }
        }
    }

    fn apply_pick(&mut self) {
        let language = self.module_menu.language.clone();
        let checked = self.module_menu.checked();
        self.notify(if checked.is_empty() {
            format!("{language}, no modules")
        } else {
            format!("{language} + {}", checked.join(", "))
        });
        self.set_modules(&language, checked);
    }

    /// Record the modules to mix in, and start a test with them if the
    /// language is the one in use.
    fn set_modules(&mut self, language: &str, checked: Vec<String>) {
        if checked.is_empty() {
            self.config.modules.remove(language);
        } else {
            self.config.modules.insert(language.to_string(), checked);
        }
        if self.config.language == language {
            self.rebuild_test();
        }
        self.save_config();
    }

    /// Download what's newly ticked, remove what's unticked, switch to
    /// the language and mix in everything ticked.
    fn apply_download(&mut self) {
        let language = self.module_menu.language.clone();
        let (fetch, remove) = self.module_menu.changes();
        let dir = modules::dir(&self.paths.languages_dir, &language);
        for m in &remove {
            if let Err(e) = catalog::uninstall(&dir, m) {
                self.notify(format!("could not remove {language}/{m}: {e}"));
            }
        }
        if !remove.is_empty() {
            self.reload_modules();
        }
        let Some(fetcher) = &mut self.fetcher else {
            self.notify("the catalogue is off (set catalog in the config)");
            return;
        };
        for m in &fetch {
            if !self
                .module_downloads
                .contains(&(language.clone(), m.clone()))
            {
                fetcher.fetch_module(language.clone(), m.clone());
                self.module_downloads.push((language.clone(), m.clone()));
            }
        }
        let checked = self.module_menu.checked();
        if self.languages.get(&language).is_some() {
            if self.config.language != language {
                self.config.language = language.clone();
            }
            self.set_modules(&language, checked);
        } else {
            // The language itself comes first; it's used once it's in.
            self.config.modules.insert(language.clone(), checked);
            self.modules_chosen_for = Some(language.clone());
            self.install(Some(Kind::Language), &language, true);
        }
        let mut note = Vec::new();
        if !fetch.is_empty() {
            note.push(format!("downloading {}…", fetch.join(", ")));
        }
        if !remove.is_empty() {
            note.push(format!("removed {}", remove.join(", ")));
        }
        if !note.is_empty() {
            self.notify_more(note.join(" · "));
        }
    }

    /// A module download finished: install it and use it if its language
    /// is the one in use.
    pub(super) fn module_fetched(
        &mut self,
        language: String,
        module: String,
        result: Result<String, String>,
    ) {
        self.module_downloads
            .retain(|(l, m)| !(*l == language && *m == module));
        let text = match result {
            Ok(t) => t,
            Err(e) => {
                self.notify(e);
                return;
            }
        };
        let dir = modules::dir(&self.paths.languages_dir, &language);
        if let Err(e) = catalog::install(&dir, &module, &text) {
            self.notify(format!("could not install {language}/{module}: {e}"));
            return;
        }
        self.reload_modules();
        if self.take_update(&format!("{language}/{module}")) {
            self.notify(format!("updated {language}/{module}"));
            if self.config.language == language {
                self.refresh_words();
            }
            return;
        }
        self.notify(format!("installed {language}/{module}"));
        let wanted = self
            .config
            .modules
            .get(&language)
            .is_some_and(|w| w.contains(&module));
        if wanted && self.config.language == language && self.engine.status() == Status::Idle {
            self.rebuild_test();
        }
    }

    pub(super) fn reload_modules(&mut self) {
        let mut warnings = Vec::new();
        self.modules = ModuleRegistry::load(&self.paths.languages_dir, |w| warnings.push(w));
        if let Some(w) = warnings.into_iter().next() {
            self.notify_more(w);
        }
    }

    /// Switching languages: offer the checklist when it has modules, unless
    /// a download for it is still under way.
    pub(super) fn offer_modules(&mut self, language: &str) {
        if self.modules_chosen_for.as_deref() == Some(language) {
            self.modules_chosen_for = None;
            return;
        }
        let downloading = self.module_downloads.iter().any(|(l, _)| l == language);
        if !downloading && !self.modules.of(language).is_empty() {
            self.open_module_pick(language);
        }
    }
}
