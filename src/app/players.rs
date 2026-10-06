//! Finding and following players: `:search [name]` and `:follow [login]`
//! open the players screen (search and following tabs), `:unfollow`, and
//! `f` there or on a profile. The follow list is fetched when logged in
//! and kept in `App::follows`, which also feeds `:follow`'s palette.

use ttyp_core::api::{PlayerList, PlayerSummary};

use super::action::PlayersAction;
use super::{App, Screen};
use crate::online::players::{PAGE, RECENT_FOLLOWS};
use crate::online::{OnlineError, PlayersTab, PlayersView, Request};

impl App {
    /// The players screen on `tab`; a search starts typing when no name
    /// was given.
    pub(super) fn open_players(&mut self, tab: PlayersTab, query: Option<String>) {
        if self.online.is_none() {
            self.notify("offline: set server in config");
            return;
        }
        let editing = tab == PlayersTab::Search && query.is_none();
        let mut view = PlayersView::new(tab, query.unwrap_or_default(), editing);
        view.following.set_len(self.follows.list().len());
        self.players = Some(view);
        self.search_players();
        self.fetch_follows();
        if self.screen != Screen::Players {
            self.players_from = match self.screen {
                Screen::Graph | Screen::User => self.previous_screen,
                s => s,
            };
        }
        self.screen = Screen::Players;
        self.scroll = 0;
    }

    /// Ask for the first page of the current query.
    fn search_players(&mut self) {
        let (Some(view), Some(online)) = (&mut self.players, &mut self.online) else {
            return;
        };
        view.set_query(view.query.clone());
        online.request(Request::Players {
            query: view.query.clone(),
            offset: 0,
            limit: PAGE,
        });
    }

    /// Refresh the follow list, when logged in.
    pub(super) fn fetch_follows(&mut self) {
        if let Some(online) = &mut self.online
            && online.logged_in()
        {
            online.request(Request::Follows);
        }
    }

    pub(super) fn players_action(&mut self, action: PlayersAction) {
        use PlayersAction as P;
        let follows_len = self.follows.list().len();
        let Some(view) = &mut self.players else {
            return;
        };
        let mut requery = false;
        match action {
            P::Up => view.selection().move_by(-1),
            P::Down => view.selection().move_by(1),
            P::Top => view.selection().home(),
            P::Bottom => view.selection().end(),
            P::PageUp => view.selection().page(-1),
            P::PageDown => view.selection().page(1),
            P::SwitchTab => {
                view.tab = match view.tab {
                    PlayersTab::Search => PlayersTab::Following,
                    PlayersTab::Following => PlayersTab::Search,
                };
                view.editing = false;
                view.following.set_len(follows_len);
            }
            P::Edit => {
                view.tab = PlayersTab::Search;
                view.editing = true;
            }
            P::StopEditing => view.editing = false,
            P::SearchChar(c) => {
                view.query.push(c);
                requery = true;
            }
            P::SearchBackspace => requery = view.query.pop().is_some(),
            P::SearchDeleteWord => {
                let trimmed = view.query.trim_end();
                let cut = trimmed.rfind(' ').map_or(0, |i| i + 1);
                requery = cut != view.query.len();
                view.query.truncate(cut);
            }
            P::Open => {
                if let Some(login) = self.selected_player().map(|p| p.login.clone()) {
                    self.open_user(Some(login));
                }
                return;
            }
            P::Follow => {
                if let Some(p) = self.selected_player() {
                    let (login, follow) = (p.login.clone(), !p.following);
                    self.set_follow(login, follow);
                }
                return;
            }
            P::Close => {
                self.players = None;
                self.screen = self.players_from;
                return;
            }
        }
        if requery {
            self.search_players();
        }
        self.fetch_more_players();
    }

    /// The row under the cursor on the tab being shown.
    pub fn selected_player(&self) -> Option<&PlayerSummary> {
        let view = self.players.as_ref()?;
        match view.tab {
            PlayersTab::Search => view.found.get(view.search.selected),
            PlayersTab::Following => self.follows.list().get(view.following.selected),
        }
    }

    fn fetch_more_players(&mut self) {
        let (Some(view), Some(online)) = (&mut self.players, &mut self.online) else {
            return;
        };
        if view.tab != PlayersTab::Search {
            return;
        }
        if let Some(offset) = view.next_page() {
            view.loading = true;
            online.request(Request::Players {
                query: view.query.clone(),
                offset,
                limit: PAGE,
            });
        }
    }

    /// `:follow <login>`: their profile if we follow them, else follow.
    pub(super) fn follow_command(&mut self, login: String) {
        match self.follows.find(&login) {
            Some(p) => {
                let login = p.login.clone();
                self.open_user(Some(login));
            }
            None => self.set_follow(login, true),
        }
    }

    /// Ask the server to follow or unfollow `login`.
    pub(super) fn set_follow(&mut self, login: String, follow: bool) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        if !online.logged_in() {
            self.notify("not logged in (:login)");
            return;
        }
        if !follow && self.follows.loaded() && !self.follows.contains(&login) {
            self.notify(format!("you don't follow {login}"));
            return;
        }
        online.request(Request::SetFollow { login, follow });
    }

    pub(super) fn players_reply(
        &mut self,
        query: &str,
        offset: u32,
        result: Result<PlayerList, OnlineError>,
    ) {
        let Some(view) = &mut self.players else {
            return;
        };
        match result {
            Ok(list) => view.apply(query, offset, list),
            Err(e) => view.fail(query, e.to_string()),
        }
        self.fetch_more_players();
    }

    pub(super) fn follows_reply(&mut self, result: Result<Vec<PlayerSummary>, OnlineError>) {
        match result {
            Ok(list) => self.set_follows(list),
            // Quietly: this runs at startup too.
            Err(OnlineError::Unauthorized) => self.follows.clear(),
            Err(e) if self.screen == Screen::Players => self.notify(format!("follows: {e}")),
            Err(_) => {}
        }
    }

    pub(super) fn followed_reply(
        &mut self,
        login: &str,
        follow: bool,
        result: Result<Vec<PlayerSummary>, OnlineError>,
    ) {
        match result {
            Ok(list) => {
                // The server's spelling of the name.
                let name = list
                    .iter()
                    .find(|p| p.login.eq_ignore_ascii_case(login))
                    .map_or(login, |p| p.login.as_str())
                    .to_string();
                self.set_follows(list);
                self.notify(if follow {
                    format!("following {name} · :follow {name} opens their profile")
                } else {
                    format!("unfollowed {name}")
                });
            }
            Err(OnlineError::Server { status: 404, .. }) => {
                self.notify(format!("{login} has no public profile"));
            }
            Err(e) => self.notify(format!("follow: {e}")),
        }
    }

    /// A new follow list: keep the screens and the palette in step.
    fn set_follows(&mut self, list: Vec<PlayerSummary>) {
        self.follows.set(list);
        self.follows_changed();
        let follows = &self.follows;
        if let Some(view) = &mut self.players {
            view.sync_following(follows);
        }
        if let Some(p) = self.user.as_mut().and_then(|v| v.profile.as_mut()) {
            p.following = follows.contains(&p.login);
        }
    }

    /// The follow list's order or contents changed.
    pub(super) fn follows_changed(&mut self) {
        self.completions.follows = self.follows.recent(RECENT_FOLLOWS);
        let len = self.follows.list().len();
        if let Some(view) = &mut self.players {
            view.following.set_len(len);
        }
    }

    /// `f` on a profile.
    pub(super) fn toggle_follow_profile(&mut self) {
        let Some(p) = self.user.as_ref().and_then(|v| v.profile.as_ref()) else {
            return;
        };
        let (login, follow) = (p.login.clone(), !p.following);
        self.set_follow(login, follow);
    }
}
