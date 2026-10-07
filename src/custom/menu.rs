//! The custom page's and the editor's state, no rendering. The page lists
//! your sets (made or installed) and then the server's shared sets, most
//! installed first. The editor names a new set, then takes words typed
//! with spaces between them and asks before adding them.

use ttyp_core::api::{CustomList, CustomSummary};

use super::{Addition, CustomSet, CustomStore, addition, valid_name};
use crate::ui::widgets::Selection;

/// Shared sets fetched per request.
pub const PAGE: u32 = 50;

/// One row of the custom page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub words: usize,
    /// Who published it; `None` for your own sets.
    pub author: Option<String>,
    /// Installs on the server, when it's published there.
    pub installs: Option<u32>,
    /// On this machine.
    pub local: bool,
}

/// What the page is asking before doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    Remove(String),
    Unpublish(String),
}

#[derive(Debug)]
pub struct CustomMenu {
    pub selection: Selection,
    /// Narrows both lists; typed after `/`.
    pub query: String,
    pub editing: bool,
    pub shared: Vec<CustomSummary>,
    pub total: u32,
    pub loading: bool,
    /// The shared list couldn't be fetched (or there's no server).
    pub error: Option<String>,
    pub confirm: Option<Confirm>,
}

impl Default for CustomMenu {
    fn default() -> Self {
        Self {
            selection: Selection::clamped(0),
            query: String::new(),
            editing: false,
            shared: Vec::new(),
            total: 0,
            loading: false,
            error: None,
            confirm: None,
        }
    }
}

impl CustomMenu {
    /// Your sets matching the search, then shared ones you don't have.
    pub fn entries(&self, store: &CustomStore) -> Vec<Entry> {
        let q = self.query.trim().to_lowercase();
        let hit = |name: &str, author: Option<&str>| {
            q.is_empty()
                || name.contains(&q)
                || author.is_some_and(|a| a.to_lowercase().contains(&q))
        };
        let published = |name: &str| self.shared.iter().find(|s| s.name == name);
        let mut v: Vec<Entry> = store
            .all()
            .filter(|s| hit(&s.name, s.author.as_deref()))
            .map(|s| Entry {
                name: s.name.clone(),
                words: s.words.len(),
                author: s.author.clone(),
                installs: published(&s.name).map(|p| p.installs),
                local: true,
            })
            .collect();
        v.extend(
            self.shared
                .iter()
                .filter(|s| !store.contains(&s.name))
                .map(|s| Entry {
                    name: s.name.clone(),
                    words: s.words as usize,
                    author: Some(s.author.clone()),
                    installs: Some(s.installs),
                    local: false,
                }),
        );
        v
    }

    pub fn selected(&self, store: &CustomStore) -> Option<Entry> {
        self.entries(store).into_iter().nth(self.selection.selected)
    }

    /// Keep the cursor inside the list after it changed.
    pub fn sync(&mut self, store: &CustomStore) {
        self.selection.set_len(self.entries(store).len());
    }

    /// A new search: the first page of shared sets is on its way.
    pub fn searching(&mut self) {
        self.loading = true;
        self.error = None;
        self.selection.select(0);
    }

    /// A page of shared sets for `query`; stale or out-of-order pages are
    /// dropped.
    pub fn apply(&mut self, query: &str, list: CustomList, store: &CustomStore) {
        if query != self.query {
            return;
        }
        if list.offset == 0 {
            self.shared = list.rows;
        } else if list.offset as usize == self.shared.len() {
            self.shared.extend(list.rows);
        } else {
            return;
        }
        self.total = list.total;
        self.loading = false;
        self.error = None;
        self.sync(store);
    }

    pub fn fail(&mut self, query: &str, e: String) {
        if query == self.query {
            self.loading = false;
            self.error = Some(e);
        }
    }

    /// The offset of the next page of shared sets, when the cursor is near
    /// the end and the server has more.
    pub fn next_page(&self) -> Option<u32> {
        let more = (self.shared.len() as u32) < self.total;
        (more && !self.loading && self.selection.near_end(10)).then_some(self.shared.len() as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditStep {
    /// Typing the new set's name.
    Name,
    /// Typing words (or moving through them, with `focus_list`).
    Words,
}

#[derive(Debug)]
pub struct CustomEditor {
    pub set: CustomSet,
    pub step: EditStep,
    /// What's being typed: the name, or words separated by spaces.
    pub input: String,
    /// Words typed and waiting for y/n.
    pub pending: Option<Addition>,
    /// The cursor is in the word list (to remove words) instead of the input.
    pub focus_list: bool,
    pub selection: Selection,
    /// The last thing that happened, for the status line.
    pub message: Option<String>,
}

impl CustomEditor {
    /// A new set: its name comes first.
    pub fn new_set() -> Self {
        Self::with(CustomSet::new(""), EditStep::Name)
    }

    /// Add to and trim an existing set.
    pub fn edit(set: CustomSet) -> Self {
        Self::with(set, EditStep::Words)
    }

    fn with(set: CustomSet, step: EditStep) -> Self {
        let n = set.words.len();
        Self {
            set,
            step,
            input: String::new(),
            pending: None,
            focus_list: false,
            selection: Selection::clamped(n),
            message: None,
        }
    }

    /// Enter on the name: take it if it's valid and free.
    pub fn take_name(&mut self, store: &CustomStore) -> Result<(), String> {
        let name = self.input.trim().to_string();
        if !valid_name(&name) {
            return Err("names are 2-32 lowercase letters, digits, _ or -".into());
        }
        if store.contains(&name) {
            return Err(format!("you already have a set called {name}"));
        }
        self.set.name = name;
        self.input.clear();
        self.step = EditStep::Words;
        Ok(())
    }

    /// Enter on typed words: sort them out and ask before adding. When
    /// nothing new is left, say why instead.
    pub fn propose(&mut self) {
        let a = addition(&self.set.words, &self.input);
        if a.new.is_empty() {
            self.message = Some(if self.input.trim().is_empty() {
                "type words separated by spaces, then enter".into()
            } else {
                format!("nothing new to add{}", notes(&a))
            });
            return;
        }
        self.pending = Some(a);
    }

    /// `y`: add the pending words. Returns how many were added.
    pub fn confirm(&mut self) -> usize {
        let Some(a) = self.pending.take() else {
            return 0;
        };
        let n = a.new.len();
        self.set.words.extend(a.new);
        self.selection.set_len(self.set.words.len());
        self.input.clear();
        self.message = Some(format!(
            "added {n} {} · {} in the set",
            if n == 1 { "word" } else { "words" },
            self.set.words.len()
        ));
        n
    }

    /// `n`: back to the typed words, unchanged.
    pub fn cancel(&mut self) {
        self.pending = None;
        self.message = Some("not added · edit the words or esc to clear".into());
    }

    /// Remove the word under the cursor; returns it.
    pub fn remove_selected(&mut self) -> Option<String> {
        let i = self.selection.selected;
        if i >= self.set.words.len() {
            return None;
        }
        let w = self.set.words.remove(i);
        self.selection.set_len(self.set.words.len());
        self.message = Some(format!("removed {w} · {} left", self.set.words.len()));
        Some(w)
    }
}

/// `(2 already in · 1 too long)` after a summary, when anything was left
/// out.
pub fn notes(a: &Addition) -> String {
    let mut parts = Vec::new();
    if !a.duplicates.is_empty() {
        parts.push(match a.duplicates.len() {
            1 => "1 duplicate".to_string(),
            n => format!("{n} duplicates"),
        });
    }
    if !a.rejected.is_empty() {
        let (w, why) = &a.rejected[0];
        parts.push(if a.rejected.len() == 1 {
            format!("{w}: {why}")
        } else {
            format!("{} can't be used ({w}: {why})", a.rejected.len())
        });
    }
    if a.over_limit > 0 {
        parts.push(format!(
            "{} over the {} word limit",
            a.over_limit,
            ttyp_core::custom::MAX_WORDS
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(name: &str, installs: u32) -> CustomSummary {
        CustomSummary {
            name: name.into(),
            author: "ann".into(),
            words: 3,
            installs,
            updated: "2026-10-07".into(),
        }
    }

    fn store_with(names: &[&str]) -> (CustomStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ttyp-custom-menu-{}-{}",
            std::process::id(),
            names.join("-")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = CustomStore::load(&dir, |_| {});
        for n in names {
            let mut s = CustomSet::new(n);
            s.words = vec!["a".into()];
            store.save(s).unwrap();
        }
        (store, dir)
    }

    #[test]
    fn the_page_lists_yours_then_shared_ones_you_lack() {
        let (store, dir) = store_with(&["birds", "rust"]);
        let mut m = CustomMenu::default();
        m.searching();
        let list = CustomList {
            rows: vec![summary("rust", 9), summary("cats", 4)],
            offset: 0,
            total: 2,
        };
        m.apply("", list, &store);
        let e = m.entries(&store);
        let names: Vec<&str> = e.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["birds", "rust", "cats"]);
        assert_eq!(e[1].installs, Some(9), "yours, and published");
        assert!(!e[2].local);
        m.query = "ca".into();
        assert_eq!(m.entries(&store).len(), 1);
        m.query = "ann".into();
        assert_eq!(m.entries(&store).len(), 1, "matches the author");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_editor_names_then_asks_before_adding() {
        let (store, dir) = store_with(&["birds"]);
        let mut ed = CustomEditor::new_set();
        ed.input = "birds".into();
        assert!(ed.take_name(&store).is_err(), "taken");
        ed.input = "Big Cats".into();
        assert!(ed.take_name(&store).is_err(), "invalid");
        ed.input = "cats".into();
        ed.take_name(&store).unwrap();
        assert_eq!((ed.set.name.as_str(), ed.step), ("cats", EditStep::Words));

        ed.input = "lion tiger lion".into();
        ed.propose();
        assert_eq!(ed.set.words.len(), 0, "nothing added before y");
        ed.cancel();
        assert_eq!(ed.input, "lion tiger lion", "the typing is kept");
        ed.propose();
        assert_eq!(ed.confirm(), 2);
        assert_eq!(ed.set.words, ["lion", "tiger"]);
        assert!(ed.input.is_empty());

        ed.input = "tiger".into();
        ed.propose();
        assert!(ed.pending.is_none());
        assert!(ed.message.as_deref().unwrap().contains("1 duplicate"));

        ed.selection.select(0);
        assert_eq!(ed.remove_selected().as_deref(), Some("lion"));
        assert_eq!(ed.set.words, ["tiger"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
