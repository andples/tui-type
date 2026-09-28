//! The app's side of online play: connecting when `server` is set, login
//! and logout, and applying replies from the worker threads.

use super::{App, Screen};
use crate::config::{Config, Paths};
use crate::online::{Client, Online, RemoteEvent, Request, token};

impl App {
    /// An `Online` when the config names a server; otherwise ttyp stays
    /// fully offline and never touches the network.
    pub(super) fn connect(config: &Config, paths: &Paths) -> Option<Online> {
        let server = config.server.as_deref()?;
        let client = Client::new(server, token::load(&paths.token_file));
        Some(Online::new(client, config.github_client_id.clone()))
    }

    /// Apply every network reply that has arrived.
    pub(super) fn drain_remote(&mut self) {
        let events = match &mut self.online {
            Some(o) => o.poll(),
            None => return,
        };
        for ev in events {
            self.dispatch(super::Action::Remote(Box::new(ev)));
        }
    }

    /// `:login`: the GitHub device flow on a background thread, with the
    /// code shown on its own screen until it completes.
    pub(super) fn login(&mut self) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        if online.github_client_id.is_none() {
            self.notify("set github_client_id in config to log in");
            return;
        }
        online.request(Request::Login);
        self.login_prompt = None;
        self.push_screen(Screen::Login);
    }

    pub(super) fn cancel_login(&mut self) {
        if let Some(o) = &self.online {
            o.cancel_login();
        }
        self.login_prompt = None;
        self.dispatch(super::Action::Back);
    }

    pub(super) fn logout(&mut self) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        if !online.logged_in() {
            self.notify("not logged in");
            return;
        }
        online.request(Request::Logout);
        online.client.set_token(None);
        if let Err(e) = token::delete(&self.paths.token_file) {
            self.notify(format!("could not remove token: {e}"));
        } else {
            self.notify("logged out");
        }
    }

    pub(super) fn remote_event(&mut self, ev: Box<RemoteEvent>) {
        match *ev {
            RemoteEvent::DeviceCode {
                user_code,
                verification_uri,
            } => self.login_prompt = Some((user_code, verification_uri)),
            RemoteEvent::LoggedIn { login, token } => {
                if let Some(o) = &mut self.online {
                    o.client.set_token(Some(token.clone()));
                }
                match token::save(&self.paths.token_file, &token) {
                    Ok(()) => self.notify(format!("logged in as {login}")),
                    Err(e) => self.notify(format!(
                        "logged in as {login}, but could not save token: {e}"
                    )),
                }
                self.leave_login();
            }
            RemoteEvent::LoginFailed(e) => {
                self.notify(format!("login failed: {e}"));
                self.leave_login();
            }
            RemoteEvent::LoggedOut => {}
            RemoteEvent::Dailies(_)
            | RemoteEvent::Daily(_)
            | RemoteEvent::Submitted { .. }
            | RemoteEvent::Leaderboard { .. }
            | RemoteEvent::ResultDetail(_) => {}
        }
    }

    fn leave_login(&mut self) {
        self.login_prompt = None;
        if self.screen == Screen::Login {
            self.dispatch(super::Action::Back);
        }
    }
}
