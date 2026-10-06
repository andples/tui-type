//! The players screen's state, no rendering: the search (`:search`) and
//! the follow list (`:follow`), plus `Follows`, our follow list ordered by
//! when we last looked at each profile (kept in the data dir).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ttyp_core::api::{PlayerList, PlayerSummary};

use crate::ui::widgets::Selection;

/// Rows per search request.
pub const PAGE: u32 = 50;
/// How many followed players the palette offers for `:follow`.
pub const RECENT_FOLLOWS: usize = 10;
/// Profile views remembered, newest kept.
const MAX_VIEWS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayersTab {
    Search,
    Following,
}

#[derive(Debug)]
pub struct PlayersView {
    pub tab: PlayersTab,
    pub query: String,
    /// Letters go to the query instead of being keys.
    pub editing: bool,
    /// Search results so far (pages are appended).
    pub found: Vec<PlayerSummary>,
    pub total: u32,
    pub loading: bool,
    pub error: Option<String>,
    pub search: Selection,
    pub following: Selection,
}

impl PlayersView {
    pub fn new(tab: PlayersTab, query: String, editing: bool) -> Self {
        Self {
            tab,
            query,
            editing,
            found: Vec::new(),
            total: 0,
            loading: true,
            error: None,
            search: Selection::clamped(0),
            following: Selection::clamped(0),
        }
    }

    pub fn selection(&mut self) -> &mut Selection {
        match self.tab {
            PlayersTab::Search => &mut self.search,
            PlayersTab::Following => &mut self.following,
        }
    }

    /// A new query: the first page is on its way.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.loading = true;
        self.error = None;
        self.search.select(0);
    }

    /// A page for `query` from `offset`; replies for an older query are
    /// dropped, and so are pages that don't continue the list.
    pub fn apply(&mut self, query: &str, offset: u32, list: PlayerList) {
        if query != self.query {
            return;
        }
        if offset == 0 {
            self.found = list.rows;
        } else if offset as usize == self.found.len() {
            self.found.extend(list.rows);
        } else {
            return;
        }
        self.total = list.total;
        self.loading = false;
        self.error = None;
        self.search.set_len(self.found.len());
    }

    pub fn fail(&mut self, query: &str, e: String) {
        if query == self.query {
            self.loading = false;
            self.error = Some(e);
        }
    }

    /// The offset of the next page to fetch, when the cursor is near the
    /// end of what's loaded and there's more.
    pub fn next_page(&self) -> Option<u32> {
        let more = (self.found.len() as u32) < self.total;
        (more && !self.loading && self.search.near_end(10)).then_some(self.found.len() as u32)
    }

    /// Mark the search's rows from the follow list.
    pub fn sync_following(&mut self, follows: &Follows) {
        for p in &mut self.found {
            p.following = follows.contains(&p.login);
        }
    }
}

/// Who we follow, most recently viewed profile first (never-viewed ones
/// after, newest follow first), and when we last opened each profile.
#[derive(Debug, Default)]
pub struct Follows {
    /// `None` until the server has answered.
    list: Option<Vec<PlayerSummary>>,
    /// Lowercase login → unix seconds of the last profile view.
    views: HashMap<String, i64>,
    path: Option<PathBuf>,
}

impl Follows {
    /// With the profile views saved at `path` (missing or bad files are
    /// an empty history).
    pub fn load(path: &Path) -> Self {
        let views = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            list: None,
            views,
            path: Some(path.to_path_buf()),
        }
    }

    pub fn loaded(&self) -> bool {
        self.list.is_some()
    }

    pub fn list(&self) -> &[PlayerSummary] {
        self.list.as_deref().unwrap_or_default()
    }

    pub fn contains(&self, login: &str) -> bool {
        self.list()
            .iter()
            .any(|p| p.login.eq_ignore_ascii_case(login))
    }

    /// The followed player called `login`, in their own spelling.
    pub fn find(&self, login: &str) -> Option<&PlayerSummary> {
        self.list()
            .iter()
            .find(|p| p.login.eq_ignore_ascii_case(login))
    }

    /// A new list from the server (in follow order, newest first).
    pub fn set(&mut self, list: Vec<PlayerSummary>) {
        self.list = Some(list);
        self.sort();
    }

    /// Logged out: nobody is followed.
    pub fn clear(&mut self) {
        self.list = None;
    }

    /// We opened `login`'s profile at `now` (unix seconds).
    pub fn viewed(&mut self, login: &str, now: i64) {
        self.views.insert(login.to_ascii_lowercase(), now);
        if self.views.len() > MAX_VIEWS {
            let mut by_age: Vec<(String, i64)> = self.views.drain().collect();
            by_age.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
            by_age.truncate(MAX_VIEWS);
            self.views = by_age.into_iter().collect();
        }
        self.sort();
        if let Some(path) = &self.path
            && let Ok(json) = serde_json::to_string(&self.views)
        {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, json);
        }
    }

    /// The `n` followed logins viewed most recently, for the palette.
    pub fn recent(&self, n: usize) -> Vec<String> {
        self.list()
            .iter()
            .take(n)
            .map(|p| p.login.clone())
            .collect()
    }

    fn sort(&mut self) {
        let views = &self.views;
        if let Some(list) = &mut self.list {
            // Stable: unviewed players keep the server's newest-first order.
            list.sort_by_key(|p| {
                std::cmp::Reverse(
                    views
                        .get(&p.login.to_ascii_lowercase())
                        .copied()
                        .unwrap_or(0),
                )
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(login: &str) -> PlayerSummary {
        PlayerSummary {
            login: login.into(),
            bests: [None; 3],
            badges: Default::default(),
            following: true,
            public: true,
        }
    }

    fn logins(f: &Follows) -> Vec<&str> {
        f.list().iter().map(|p| p.login.as_str()).collect()
    }

    #[test]
    fn follows_are_ordered_by_last_view() {
        let dir = std::env::temp_dir().join(format!("ttyp-views-{}", std::process::id()));
        let path = dir.join("views.json");
        let mut f = Follows::load(&path);
        assert!(!f.loaded());
        f.viewed("Cy", 50);
        f.set(vec![
            player("ann"),
            player("bo"),
            player("cy"),
            player("di"),
        ]);
        assert_eq!(logins(&f), ["cy", "ann", "bo", "di"]);
        f.viewed("di", 100);
        f.viewed("BO", 90);
        assert_eq!(logins(&f), ["di", "bo", "cy", "ann"]);
        assert_eq!(f.recent(2), ["di", "bo"]);
        assert!(f.contains("ANN") && !f.contains("eve"));
        assert_eq!(f.find("DI").map(|p| p.login.as_str()), Some("di"));
        // The views survive a restart; the list comes from the server.
        let mut again = Follows::load(&path);
        again.set(vec![
            player("ann"),
            player("bo"),
            player("cy"),
            player("di"),
        ]);
        assert_eq!(logins(&again), ["di", "bo", "cy", "ann"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn search_pages_append_and_old_queries_are_dropped() {
        let mut v = PlayersView::new(PlayersTab::Search, String::new(), true);
        v.set_query("an".into());
        let page = |rows: &[&str], offset, total| PlayerList {
            rows: rows.iter().map(|l| player(l)).collect(),
            offset,
            total,
        };
        v.apply("a", 0, page(&["zed"], 0, 1));
        assert!(v.found.is_empty() && v.loading, "stale reply ignored");
        v.apply("an", 0, page(&["ann", "anna"], 0, 3));
        assert_eq!(v.found.len(), 2);
        assert_eq!(v.next_page(), Some(2));
        v.loading = true;
        assert_eq!(v.next_page(), None, "one page at a time");
        v.apply("an", 2, page(&["joanne"], 2, 3));
        assert_eq!(v.found.len(), 3);
        assert_eq!(v.next_page(), None, "all loaded");
        let mut f = Follows::default();
        f.set(vec![player("Anna")]);
        v.sync_following(&f);
        let flags: Vec<bool> = v.found.iter().map(|p| p.following).collect();
        assert_eq!(flags, [false, true, false]);
    }
}
