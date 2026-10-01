//! The install screen's behaviour: fetching the catalogue index,
//! installing, switching to and removing languages and themes.

use super::action::{Action, CatalogAction};
use super::{App, Screen};
use crate::catalog::{self, CatalogEvent, Fetcher, Kind, menu::IndexState};
use crate::command::Command;
use crate::language::LanguageRegistry;
use crate::test::Status;
use crate::theme::ThemeRegistry;

impl App {
    /// A `Fetcher` when the config names a catalogue.
    pub(super) fn catalog_fetcher(config: &crate::config::Config) -> Option<Fetcher> {
        config.catalog_source().map(Fetcher::new)
    }

    pub(super) fn open_catalog(&mut self) {
        self.catalog_menu.confirm_remove = false;
        self.catalog_menu.query.clear();
        self.load_index(false);
        self.sync_catalog();
        // Start on the language and theme in use.
        for (kind, current) in [
            (Kind::Language, self.config.language.clone()),
            (Kind::Theme, self.config.theme.clone()),
        ] {
            let items = self.catalog_menu.items(kind, &self.themes, &self.languages);
            self.catalog_menu.select_name(kind, &current, &items);
        }
        self.push_screen(Screen::Catalog);
    }

    /// Fetch the index unless it's loaded or on its way (`force` refetches).
    fn load_index(&mut self, force: bool) {
        let menu = &mut self.catalog_menu;
        let Some(fetcher) = &mut self.fetcher else {
            menu.index = IndexState::Disabled;
            return;
        };
        let stale = matches!(
            menu.index,
            IndexState::NotLoaded | IndexState::Failed(_) | IndexState::Disabled
        );
        if (force && menu.index != IndexState::Loading) || stale {
            menu.index = IndexState::Loading;
            fetcher.fetch_index();
        }
    }

    /// Apply every catalogue download that has finished.
    pub(super) fn drain_catalog(&mut self) {
        let events = match &mut self.fetcher {
            Some(f) => f.poll(),
            None => return,
        };
        for ev in events {
            self.dispatch(Action::CatalogFetched(Box::new(ev)));
        }
    }

    pub(super) fn catalog_action(&mut self, action: CatalogAction) {
        use CatalogAction as C;
        let menu = &self.catalog_menu;
        let len = menu.items(menu.tab, &self.themes, &self.languages).len();
        let selected = menu.selected(&self.themes, &self.languages);
        let kind = menu.tab;
        match action {
            C::Up => self.catalog_menu.move_by(-1, len),
            C::Down => self.catalog_menu.move_by(1, len),
            C::Top => self.catalog_menu.jump(false, len),
            C::Bottom => self.catalog_menu.jump(true, len),
            C::SwitchTab => {
                self.catalog_menu.switch_tab();
                self.sync_catalog();
            }
            C::Use | C::Install => {
                let Some(item) = selected else { return };
                let has_modules = kind == Kind::Language
                    && self
                        .catalog_menu
                        .index()
                        .and_then(|i| i.language(&item.name))
                        .is_some_and(|l| !l.modules.is_empty());
                if action == C::Use && has_modules {
                    self.open_module_download(&item.name);
                    return;
                }
                if item.status.is_installed() {
                    if action == C::Use {
                        self.use_item(kind, &item.name);
                    } else {
                        self.notify(format!("{kind} {} is already installed", item.name));
                    }
                } else {
                    self.install(Some(kind), &item.name, action == C::Use);
                }
            }
            C::Remove => match selected {
                Some(item) if item.status.removable() => self.catalog_menu.confirm_remove = true,
                Some(item) if item.status.is_installed() => {
                    self.notify(format!("{kind} {} is built in", item.name));
                }
                _ => {}
            },
            C::CancelRemove => self.catalog_menu.confirm_remove = false,
            C::ConfirmRemove => {
                self.catalog_menu.confirm_remove = false;
                if let Some(item) = selected {
                    self.uninstall(Some(kind), &item.name);
                }
            }
            C::Refresh => self.load_index(true),
            C::SearchChar(ch) => {
                self.catalog_menu
                    .search(|q| q.push(ch), &self.themes, &self.languages);
            }
            C::SearchBackspace => {
                self.catalog_menu.search(
                    |q| {
                        q.pop();
                    },
                    &self.themes,
                    &self.languages,
                );
            }
            C::SearchDeleteWord => {
                self.catalog_menu.search(
                    |q| {
                        let keep = q.trim_end().rfind(' ').map_or(0, |i| i + 1);
                        q.truncate(keep);
                    },
                    &self.themes,
                    &self.languages,
                );
            }
            C::Escape if self.catalog_menu.query.is_empty() => self.dispatch(Action::Back),
            C::Escape => {
                self.catalog_menu
                    .search(String::clear, &self.themes, &self.languages);
            }
        }
    }

    pub(super) fn catalog_event(&mut self, ev: CatalogEvent) {
        match ev {
            CatalogEvent::Index(Ok(index)) => {
                self.catalog_menu
                    .set_index(index, &self.themes, &self.languages);
                self.refresh_catalog_lists();
                self.update_stale();
            }
            CatalogEvent::Index(Err(e)) => self.catalog_menu.index = IndexState::Failed(e),
            CatalogEvent::ModuleFetched {
                language,
                module,
                result,
            } => self.module_fetched(language, module, result),
            CatalogEvent::Fetched {
                name,
                use_it,
                result,
                ..
            } => {
                self.catalog_menu.installing.retain(|(_, n)| *n != name);
                match result {
                    Ok((kind, text)) => self.finish_install(kind, &name, &text, use_it),
                    Err(e) => self.notify(e),
                }
                self.sync_catalog();
            }
        }
    }

    /// Download `name` in the background; `use_it` switches to it once
    /// it's in. Without a `kind` the catalogue index decides.
    pub(super) fn install(&mut self, kind: Option<Kind>, name: &str, use_it: bool) {
        let kind = kind.or_else(|| self.catalog_menu.index()?.kind_of(name));
        if let Some(k) = kind.filter(|k| self.is_installed(*k, name)) {
            if use_it {
                self.use_item(k, name);
            } else {
                self.notify(format!("{k} {name} is already installed"));
            }
            return;
        }
        if !catalog::valid_name(name) {
            self.notify(format!("no language or theme called `{name}`"));
            return;
        }
        let Some(fetcher) = &mut self.fetcher else {
            self.notify("the catalogue is off (set catalog in the config)");
            return;
        };
        if self.catalog_menu.installing.iter().any(|(_, n)| n == name) {
            return;
        }
        fetcher.fetch(kind, name.to_string(), use_it);
        if let Some(k) = kind {
            self.catalog_menu.installing.push((k, name.to_string()));
        }
        self.notify(format!("installing {name}…"));
    }

    fn finish_install(&mut self, kind: Kind, name: &str, text: &str, use_it: bool) {
        let dir = self.catalog_dir(kind).to_path_buf();
        if let Err(e) = catalog::install(&dir, name, text) {
            self.notify(format!("could not install {kind} {name}: {e}"));
            return;
        }
        self.reload_catalogued();
        if self.take_update(name) {
            self.notify(format!("updated {kind} {name}"));
            if kind == Kind::Language && self.config.language == name {
                self.refresh_words();
            }
            return;
        }
        self.notify(format!("installed {kind} {name}"));
        if use_it {
            self.use_item(kind, name);
            return;
        }
        // The config already asked for it (an old config, or a profile):
        // start using it now, unless a test is under way.
        let in_use = match kind {
            Kind::Language => self.config.language == name,
            Kind::Theme => self.config.theme == name,
        };
        if in_use && kind == Kind::Language && self.engine.status() == Status::Idle {
            self.rebuild_test();
        }
    }

    /// `:uninstall`, or `d` on the install screen. Moves off the removed
    /// language or theme if it was in use.
    pub(super) fn uninstall(&mut self, kind: Option<Kind>, name: &str) {
        let Some(kind) =
            kind.or_else(|| Kind::ALL.into_iter().find(|k| self.is_installed(*k, name)))
        else {
            self.notify(format!("`{name}` isn't installed"));
            return;
        };
        let builtin = match kind {
            Kind::Language => LanguageRegistry::is_builtin(name),
            Kind::Theme => ThemeRegistry::is_builtin(name),
        };
        if builtin {
            self.notify(format!("{kind} {name} is built in"));
            return;
        }
        let dir = self.catalog_dir(kind).to_path_buf();
        match catalog::uninstall(&dir, name) {
            Ok(0) => {
                self.notify(format!("`{name}` isn't installed"));
                return;
            }
            Ok(_) => {}
            Err(e) => {
                self.notify(format!("could not remove {kind} {name}: {e}"));
                return;
            }
        }
        self.reload_catalogued();
        self.notify(format!("removed {kind} {name}"));
        match kind {
            Kind::Theme if self.config.theme == name => {
                self.config.theme = "default".into();
                self.notify_more("theme default");
                self.save_config();
            }
            Kind::Language => {
                // Its modules go with it.
                let dir = crate::catalog::modules::dir(&self.paths.languages_dir, name);
                let _ = std::fs::remove_dir_all(dir);
                self.config.modules.remove(name);
                self.reload_modules();
                if self.config.language == name {
                    self.config.language = "english".into();
                    self.notify_more("language english");
                    self.rebuild_test();
                }
                self.save_config();
            }
            _ => {}
        }
        self.sync_catalog();
    }

    fn use_item(&mut self, kind: Kind, name: &str) {
        match kind {
            Kind::Language => self.execute(Command::Language(name.to_string())),
            Kind::Theme => self.execute(Command::Theme(name.to_string())),
        }
        self.notify_more(format!("{kind} {name}"));
    }

    fn is_installed(&self, kind: Kind, name: &str) -> bool {
        match kind {
            Kind::Language => self.languages.get(name).is_some(),
            Kind::Theme => self.themes.get(name).is_some(),
        }
    }

    fn catalog_dir(&self, kind: Kind) -> &std::path::Path {
        match kind {
            Kind::Language => &self.paths.languages_dir,
            Kind::Theme => &self.paths.themes_dir,
        }
    }

    /// Re-read the config dirs after an install or removal.
    fn reload_catalogued(&mut self) {
        let mut warnings = Vec::new();
        self.themes = ThemeRegistry::load(&self.paths.themes_dir, |w| warnings.push(w));
        self.languages = LanguageRegistry::load(&self.paths.languages_dir, |w| warnings.push(w));
        self.refresh_catalog_lists();
        if let Some(w) = warnings.into_iter().next() {
            self.notify_more(w);
        }
    }

    /// Palette completions that depend on what's installed.
    pub(super) fn refresh_catalog_lists(&mut self) {
        let (themes, languages) = (&self.themes, &self.languages);
        let c = &mut self.completions;
        c.themes = themes.names().map(str::to_string).collect();
        c.languages = languages.names().map(str::to_string).collect();
        c.removable = languages
            .names()
            .filter(|n| !LanguageRegistry::is_builtin(n))
            .chain(themes.names().filter(|n| !ThemeRegistry::is_builtin(n)))
            .map(str::to_string)
            .collect();
        c.installable = match self.catalog_menu.index() {
            Some(index) => Kind::ALL
                .into_iter()
                .flat_map(|k| {
                    index
                        .names(k)
                        .into_iter()
                        .filter(move |n| match k {
                            Kind::Language => languages.get(n).is_none(),
                            Kind::Theme => themes.get(n).is_none(),
                        })
                        .map(str::to_string)
                })
                .collect(),
            None => Vec::new(),
        };
        self.sync_catalog();
    }

    fn sync_catalog(&mut self) {
        self.catalog_menu.sync(&self.themes, &self.languages);
    }

    /// At startup: fetch the catalogue index, so installed languages and
    /// modules that changed in the catalogue get updated (`update_stale`).
    /// Skipped when the user has opted out of the network (`server = ""`).
    pub(super) fn check_catalog_updates(&mut self) {
        if self.config.server_url().is_some() {
            self.load_index(false);
        }
    }

    /// Refetch every installed catalogue language and module whose file
    /// isn't the catalogue's current one (by `catalog::checksum`). Files
    /// the index has no checksum for are left alone.
    fn update_stale(&mut self) {
        let Some(index) = self.catalog_menu.index() else {
            return;
        };
        let dir = &self.paths.languages_dir;
        let stale = |path: std::path::PathBuf, sum: &Option<String>| {
            let Some(sum) = sum else { return false };
            std::fs::read_to_string(path).is_ok_and(|text| catalog::checksum(&text) != *sum)
        };
        let mut languages = Vec::new();
        let mut modules = Vec::new();
        for entry in &index.languages {
            if LanguageRegistry::is_builtin(&entry.name) {
                continue;
            }
            if stale(dir.join(format!("{}.toml", entry.name)), &entry.checksum) {
                languages.push(entry.name.clone());
            }
            let mdir = crate::catalog::modules::dir(dir, &entry.name);
            for m in &entry.modules {
                if stale(mdir.join(format!("{}.toml", m.name)), &m.checksum) {
                    modules.push((entry.name.clone(), m.name.clone()));
                }
            }
        }
        let Some(fetcher) = &mut self.fetcher else {
            return;
        };
        for name in languages {
            if self.catalog_menu.installing.iter().any(|(_, n)| *n == name) {
                continue;
            }
            fetcher.fetch(Some(Kind::Language), name.clone(), false);
            self.catalog_menu
                .installing
                .push((Kind::Language, name.clone()));
            self.catalog_updates.push(name);
        }
        for (language, module) in modules {
            let key = (language.clone(), module.clone());
            if self.module_downloads.contains(&key) {
                continue;
            }
            fetcher.fetch_module(language.clone(), module.clone());
            self.module_downloads.push(key);
            self.catalog_updates.push(format!("{language}/{module}"));
        }
    }

    /// Whether `name` (a language, or `language/module`) was being
    /// updated rather than installed; forgets it either way.
    pub(super) fn take_update(&mut self, name: &str) -> bool {
        let before = self.catalog_updates.len();
        self.catalog_updates.retain(|n| n != name);
        self.catalog_updates.len() != before
    }

    /// New words for the current language without leaving the screen: an
    /// update that lands while nothing is being typed.
    pub(super) fn refresh_words(&mut self) {
        if self.engine.status() == Status::Idle && self.daily.is_none() {
            self.engine = Self::build_engine(&self.config, &self.languages, &self.modules);
        }
    }

    /// At startup: fetch a language or theme the config names but that
    /// isn't installed (a config from before they moved to the catalogue).
    /// Skipped when the user has opted out of the network (`server = ""`).
    pub(super) fn install_missing(&mut self) -> bool {
        if self.config.server_url().is_none() {
            return false;
        }
        let mut any = false;
        for (kind, name) in [
            (Kind::Theme, self.config.theme.clone()),
            (Kind::Language, self.config.language.clone()),
        ] {
            if !self.is_installed(kind, &name) && catalog::valid_name(&name) {
                self.install(Some(kind), &name, false);
                any = true;
            }
        }
        any
    }
}
