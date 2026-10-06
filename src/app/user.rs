//! Profiles: `:user [login]` (also `p` on a leaderboard row, enter or `p`
//! on the players screen) and `:account [public on|off]`.

use ttyp_core::api::{Account, Profile};

use super::action::UserAction;
use super::{App, Screen};
use crate::online::{OnlineError, Request, UserView};

impl App {
    /// Open `login`'s profile, or ours when `None`.
    pub(super) fn open_user(&mut self, login: Option<String>) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        let login = match login {
            Some(l) => l,
            // We only know our login from the server; ask, then open.
            None if online.logged_in() => {
                self.own_profile_pending = true;
                online.request(Request::Account);
                return;
            }
            None => {
                self.notify("not logged in (:login), or give a name: :user <login>");
                return;
            }
        };
        online.request(Request::User(login.clone()));
        self.follows.viewed(&login, chrono::Utc::now().timestamp());
        self.follows_changed();
        self.user = Some(UserView::new(login));
        // Esc goes back to whatever opened it (the graph goes back past
        // itself, since its run is gone by then).
        match self.screen {
            Screen::User => {}
            Screen::Graph => self.user_from = self.previous_screen,
            s => self.user_from = s,
        }
        self.screen = Screen::User;
        self.scroll = 0;
    }

    pub(super) fn user_action(&mut self, action: UserAction) {
        use UserAction as U;
        if action == U::Close {
            self.user = None;
            self.screen = self.user_from;
            return;
        }
        let Some(view) = &mut self.user else { return };
        match action {
            U::Up => view.selection.move_by(-1),
            U::Down => view.selection.move_by(1),
            U::Top => view.selection.home(),
            U::Bottom => view.selection.end(),
            U::Open => {
                let id = view.selected().map(|r| r.result_id);
                if let (Some(id), Some(o)) = (id, &mut self.online) {
                    self.graph_loading = true;
                    o.request(Request::Result(id));
                }
            }
            U::Follow => self.toggle_follow_profile(),
            U::Close => {}
        }
    }

    /// `:account`: show it, or set whether the profile is public.
    pub(super) fn account(&mut self, public: Option<bool>) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        if !online.logged_in() {
            self.notify("not logged in (:login)");
            return;
        }
        online.request(match public {
            Some(p) => Request::SetPublic(p),
            None => Request::Account,
        });
    }

    pub(super) fn account_reply(&mut self, result: Result<Account, OnlineError>) {
        let open_own = std::mem::take(&mut self.own_profile_pending);
        match result {
            Ok(a) if open_own => self.open_user(Some(a.login)),
            Ok(a) => {
                let state = if a.public {
                    "public: anyone can see it with :user"
                } else {
                    "private: only you can see it (:account public on)"
                };
                self.notify(format!("logged in as {} · profile {state}", a.login));
                // Keep an open own profile in step.
                if let Some(p) = self
                    .user
                    .as_mut()
                    .and_then(|v| v.profile.as_mut())
                    .filter(|p| p.login == a.login)
                {
                    p.public = a.public;
                }
            }
            Err(e) => self.notify(format!("account: {e}")),
        }
    }

    pub(super) fn user_reply(&mut self, login: &str, result: Result<Profile, OnlineError>) {
        let Some(view) = self.user.as_mut().filter(|v| v.wants(login)) else {
            return;
        };
        match result {
            Ok(p) => view.set(p),
            Err(OnlineError::Server { status: 404, .. }) => {
                view.fail(format!("{login} has no public profile"));
            }
            Err(e) => view.fail(e.to_string()),
        }
    }
}
