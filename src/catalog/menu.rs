//! State for the `:install` screen: what's installed and what the
//! catalogue offers, one tab per kind. UI-free; `ui::catalog` draws it.

use std::collections::BTreeMap;

use super::{Index, Kind};
use crate::language::LanguageRegistry;
use crate::theme::{Theme, ThemeRegistry};
use crate::ui::widgets::Selection;

/// Where the catalogue index is at.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum IndexState {
    #[default]
    NotLoaded,
    Loading,
    Ready(Index),
    Failed(String),
    /// `catalog` is empty in the config: nothing to fetch from.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    /// Compiled in; can't be removed.
    BuiltIn,
    /// Installed from the catalogue.
    Installed,
    /// A file in the config dir the catalogue doesn't know (hand-written,
    /// or the index isn't loaded yet).
    Local,
    Available,
    Installing,
}

impl ItemStatus {
    pub fn is_installed(self) -> bool {
        matches!(
            self,
            ItemStatus::BuiltIn | ItemStatus::Installed | ItemStatus::Local
        )
    }

    pub fn removable(self) -> bool {
        matches!(self, ItemStatus::Installed | ItemStatus::Local)
    }

    pub fn label(self) -> &'static str {
        match self {
            ItemStatus::BuiltIn => "built in",
            ItemStatus::Installed => "installed",
            ItemStatus::Local => "local",
            ItemStatus::Available => "available",
            ItemStatus::Installing => "installing…",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    /// Languages: label and word count.
    pub detail: String,
    pub status: ItemStatus,
}

pub struct CatalogMenu {
    pub tab: Kind,
    languages: Selection,
    themes: Selection,
    /// Asking before removing the selected item.
    pub confirm_remove: bool,
    pub index: IndexState,
    /// Downloads in flight.
    pub installing: Vec<(Kind, String)>,
    /// Typed search: both tabs list only what matches.
    pub query: String,
}

impl Default for CatalogMenu {
    fn default() -> Self {
        Self {
            tab: Kind::Language,
            languages: Selection::wrapping(0),
            themes: Selection::wrapping(0),
            confirm_remove: false,
            index: IndexState::NotLoaded,
            installing: Vec::new(),
            query: String::new(),
        }
    }
}

impl CatalogMenu {
    pub fn selection(&self, kind: Kind) -> &Selection {
        match kind {
            Kind::Language => &self.languages,
            Kind::Theme => &self.themes,
        }
    }

    fn selection_mut(&mut self, kind: Kind) -> &mut Selection {
        match kind {
            Kind::Language => &mut self.languages,
            Kind::Theme => &mut self.themes,
        }
    }

    pub fn index(&self) -> Option<&Index> {
        match &self.index {
            IndexState::Ready(i) => Some(i),
            _ => None,
        }
    }

    pub fn is_installing(&self, kind: Kind, name: &str) -> bool {
        self.installing.iter().any(|(k, n)| *k == kind && n == name)
    }

    /// The rows of `kind`'s tab: `all_items` narrowed by the search.
    pub fn items(&self, kind: Kind, themes: &ThemeRegistry, langs: &LanguageRegistry) -> Vec<Item> {
        let mut items = self.all_items(kind, themes, langs);
        items.retain(|i| matches(&self.query, i));
        items
    }

    /// Change the search with `edit`, then start both tabs at the top.
    pub fn search(
        &mut self,
        edit: impl FnOnce(&mut String),
        themes: &ThemeRegistry,
        langs: &LanguageRegistry,
    ) {
        self.confirm_remove = false;
        edit(&mut self.query);
        for kind in Kind::ALL {
            let len = self.items(kind, themes, langs).len();
            let sel = self.selection_mut(kind);
            sel.set_len(len);
            sel.home();
        }
    }

    /// Everything installed plus everything the catalogue offers, by name.
    pub fn all_items(
        &self,
        kind: Kind,
        themes: &ThemeRegistry,
        langs: &LanguageRegistry,
    ) -> Vec<Item> {
        let index = self.index();
        let in_catalogue = |name: &str| index.is_some_and(|i| i.contains(kind, name));
        let mut items: BTreeMap<String, Item> = BTreeMap::new();
        let installed: Vec<(String, String, bool)> = match kind {
            Kind::Language => langs
                .names()
                .map(|n| {
                    let l = langs.get(n).expect("listed");
                    let detail = format!("{} · {} words", l.display, l.words.len());
                    (n.to_string(), detail, LanguageRegistry::is_builtin(n))
                })
                .collect(),
            Kind::Theme => themes
                .names()
                .map(|n| (n.to_string(), String::new(), ThemeRegistry::is_builtin(n)))
                .collect(),
        };
        for (name, detail, builtin) in installed {
            let status = if builtin {
                ItemStatus::BuiltIn
            } else if in_catalogue(&name) {
                ItemStatus::Installed
            } else {
                ItemStatus::Local
            };
            items.insert(
                name.clone(),
                Item {
                    name,
                    detail,
                    status,
                },
            );
        }
        if let Some(index) = index {
            let offered: Vec<(String, String)> = match kind {
                Kind::Language => index
                    .languages
                    .iter()
                    .map(|l| (l.name.clone(), format!("{} · {} words", l.display, l.words)))
                    .collect(),
                Kind::Theme => index
                    .themes
                    .iter()
                    .map(|t| (t.name.clone(), String::new()))
                    .collect(),
            };
            for (name, detail) in offered {
                items.entry(name.clone()).or_insert_with(|| Item {
                    status: if self.is_installing(kind, &name) {
                        ItemStatus::Installing
                    } else {
                        ItemStatus::Available
                    },
                    name,
                    detail,
                });
            }
        }
        items.into_values().collect()
    }

    /// The highlighted item on the current tab.
    pub fn selected(&self, themes: &ThemeRegistry, langs: &LanguageRegistry) -> Option<Item> {
        let sel = self.selection(self.tab).selected;
        self.items(self.tab, themes, langs).into_iter().nth(sel)
    }

    /// Move the cursor on the current tab, given the tab's length.
    pub fn move_by(&mut self, delta: isize, len: usize) {
        self.confirm_remove = false;
        let sel = self.selection_mut(self.tab);
        sel.set_len(len);
        sel.move_by(delta);
    }

    /// Jump to the first (`end = false`) or last row of the current tab.
    pub fn jump(&mut self, end: bool, len: usize) {
        self.confirm_remove = false;
        let sel = self.selection_mut(self.tab);
        sel.set_len(len);
        if end { sel.end() } else { sel.home() }
    }

    pub fn switch_tab(&mut self) {
        self.confirm_remove = false;
        self.tab = self.tab.other();
    }

    /// Put the cursor on `name` in `kind`'s tab, if it's listed.
    pub fn select_name(&mut self, kind: Kind, name: &str, items: &[Item]) {
        let sel = self.selection_mut(kind);
        sel.set_len(items.len());
        if let Some(i) = items.iter().position(|it| it.name == name) {
            sel.select(i);
        }
    }

    /// Take a freshly loaded index, keeping each cursor on the same name
    /// (new entries shift the lists).
    pub fn set_index(&mut self, index: Index, themes: &ThemeRegistry, langs: &LanguageRegistry) {
        let before: Vec<(Kind, Option<String>)> = Kind::ALL
            .into_iter()
            .map(|k| {
                let sel = self.selection(k).selected;
                (
                    k,
                    self.items(k, themes, langs)
                        .into_iter()
                        .nth(sel)
                        .map(|i| i.name),
                )
            })
            .collect();
        self.index = IndexState::Ready(index);
        for (kind, name) in before {
            let items = self.items(kind, themes, langs);
            self.selection_mut(kind).set_len(items.len());
            if let Some(name) = name {
                self.select_name(kind, &name, &items);
            }
        }
    }

    /// Keep both cursors in range after the lists changed.
    pub fn sync(&mut self, themes: &ThemeRegistry, langs: &LanguageRegistry) {
        for kind in Kind::ALL {
            let len = self.items(kind, themes, langs).len();
            self.selection_mut(kind).set_len(len);
        }
    }

    /// The theme to preview while browsing the themes tab: the installed
    /// one, or the catalogue's copy of one that isn't installed yet.
    pub fn preview_theme<'a>(
        &'a self,
        themes: &'a ThemeRegistry,
        langs: &LanguageRegistry,
    ) -> Option<&'a Theme> {
        if self.tab != Kind::Theme {
            return None;
        }
        let item = self.selected(themes, langs)?;
        themes
            .get(&item.name)
            .or_else(|| self.index()?.theme(&item.name))
    }
}

/// Every word of `query` appears in the item's name or detail, ignoring
/// case; an empty query matches everything.
fn matches(query: &str, item: &Item) -> bool {
    let hay = format!("{} {}", item.name, item.detail).to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|w| hay.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::LanguageEntry;

    fn menu_with_index() -> CatalogMenu {
        let nord = Theme::parse(
            "name = \"nord\"\n[colors]\nbg=\"#2e3440\"\nfg=\"#d8dee9\"\nsub=\"#616e88\"\nmain=\"#88c0d0\"\ncorrect=\"#d8dee9\"\nerror=\"#bf616a\"\nerror_extra=\"#d08770\"\n",
        )
        .unwrap();
        CatalogMenu {
            index: IndexState::Ready(Index {
                languages: vec![LanguageEntry {
                    name: "spanish".into(),
                    display: "Spanish".into(),
                    words: 200,
                    modules: Vec::new(),
                }],
                themes: vec![nord],
            }),
            ..Default::default()
        }
    }

    #[test]
    fn search_narrows_both_tabs_and_resets_the_cursor() {
        let mut m = menu_with_index();
        let (themes, langs) = (ThemeRegistry::builtin(), LanguageRegistry::builtin());
        let names = |m: &CatalogMenu, k| -> Vec<String> {
            m.items(k, &themes, &langs)
                .into_iter()
                .map(|i| i.name)
                .collect()
        };
        m.move_by(2, 3);
        m.search(|q| q.push_str("SPAN"), &themes, &langs);
        assert_eq!(names(&m, Kind::Language), ["spanish"]);
        assert_eq!(m.selection(Kind::Language).selected, 0);
        assert!(names(&m, Kind::Theme).is_empty());
        // Words match anywhere in the name or detail.
        m.search(|q| *q = "english 1k".into(), &themes, &langs);
        assert_eq!(names(&m, Kind::Language), ["english_1k"]);
        m.search(String::clear, &themes, &langs);
        assert_eq!(names(&m, Kind::Language).len(), 3);
        assert_eq!(m.all_items(Kind::Theme, &themes, &langs).len(), 2);
    }

    #[test]
    fn merges_installed_and_available() {
        let mut m = menu_with_index();
        let (themes, langs) = (ThemeRegistry::builtin(), LanguageRegistry::builtin());
        let items = m.items(Kind::Language, &themes, &langs);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["english", "english_1k", "spanish"]);
        assert_eq!(items[0].status, ItemStatus::BuiltIn);
        assert_eq!(items[2].status, ItemStatus::Available);
        assert_eq!(items[2].detail, "Spanish · 200 words");

        m.installing.push((Kind::Language, "spanish".into()));
        assert_eq!(
            m.items(Kind::Language, &themes, &langs)[2].status,
            ItemStatus::Installing
        );

        let mut langs = langs;
        langs.insert(
            crate::language::Language::parse("name = \"spanish\"\nwords = [\"y\"]").unwrap(),
        );
        langs.insert(crate::language::Language::parse("name = \"mine\"\nwords = [\"y\"]").unwrap());
        let items = m.items(Kind::Language, &themes, &langs);
        let status = |n: &str| items.iter().find(|i| i.name == n).unwrap().status;
        assert_eq!(status("spanish"), ItemStatus::Installed);
        assert_eq!(status("mine"), ItemStatus::Local);
    }

    #[test]
    fn cursor_stays_on_its_item_when_the_index_arrives() {
        let (themes, langs) = (ThemeRegistry::builtin(), LanguageRegistry::builtin());
        let mut m = CatalogMenu::default();
        m.sync(&themes, &langs);
        m.move_by(1, 2);
        let index = menu_with_index().index().unwrap().clone();
        m.set_index(index, &themes, &langs);
        assert_eq!(m.selected(&themes, &langs).unwrap().name, "english_1k");
    }

    #[test]
    fn previews_themes_before_install() {
        let mut m = menu_with_index();
        let (themes, langs) = (ThemeRegistry::builtin(), LanguageRegistry::builtin());
        assert!(m.preview_theme(&themes, &langs).is_none(), "languages tab");
        m.switch_tab();
        m.sync(&themes, &langs);
        assert_eq!(m.preview_theme(&themes, &langs).unwrap().name, "default");
        m.move_by(1, 2);
        assert_eq!(m.preview_theme(&themes, &langs).unwrap().name, "nord");
    }
}
