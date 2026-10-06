//! The app's side of online play: connecting when `server` is set, login
//! and logout, and applying replies from the worker threads.

use std::path::Path;

use ttyp_core::api::{Daily, SubmitRequest};
use ttyp_core::test::{FixedGenerator, Mode, TestEngine};

use super::{App, DailyOutcome, DailyStart, DailyStatus, Screen};
use crate::config::{Config, Paths};
use crate::online::{Client, Online, OnlineError, RemoteEvent, Request, queue, token};

/// The UTC day dailies are keyed by.
fn today() -> String {
    chrono::Utc::now().date_naive().to_string()
}

impl App {
    /// An `Online` when the config names a server; otherwise ttyp stays
    /// fully offline and never touches the network.
    pub(super) fn connect(config: &Config, paths: &Paths) -> Option<Online> {
        let server = config.server_url()?;
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
        self.follows.clear();
        self.follows_changed();
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
                self.fetch_follows();
                self.leave_login();
            }
            RemoteEvent::LoginFailed(e) => {
                self.notify(format!("login failed: {e}"));
                self.leave_login();
            }
            RemoteEvent::LoggedOut => {}
            RemoteEvent::Dailies(Ok(list)) => {
                if let Some(o) = &mut self.online {
                    o.dailies = Some((today(), list));
                }
                if let Some((language, mode)) = self.pending_daily.take() {
                    self.pick_daily(&language, mode);
                }
            }
            RemoteEvent::Dailies(Err(e)) => {
                self.pending_daily = None;
                self.notify(format!("daily: {e}"));
            }
            RemoteEvent::Daily(Ok(d)) => self.start_daily(d),
            RemoteEvent::Daily(Err(e)) => self.notify(format!("daily: {e}")),
            RemoteEvent::Started { daily_id, result } => self.started(daily_id, result),
            RemoteEvent::Submitted {
                body,
                date,
                queued,
                result,
            } => self.submitted(body, date, queued.as_deref(), result),
            RemoteEvent::DailiesFor { date, result } => self.board_dailies(date, result),
            RemoteEvent::Leaderboard {
                board,
                target,
                offset,
                result,
            } => self.board_page(board, &target, offset, result),
            RemoteEvent::ResultDetail(result) => self.board_result(result),
            RemoteEvent::Account(result) => self.account_reply(result),
            RemoteEvent::User { login, result } => self.user_reply(&login, result),
            RemoteEvent::Players {
                query,
                offset,
                result,
            } => self.players_reply(&query, offset, result),
            RemoteEvent::Follows(result) => self.follows_reply(result),
            RemoteEvent::Followed {
                login,
                follow,
                result,
            } => self.followed_reply(&login, follow, result),
        }
    }

    /// `:daily [mode]`: today's daily for the current language, fetching
    /// the day's list first if needed.
    pub(super) fn open_daily(&mut self, mode: Option<Mode>) {
        if self.refuse_if_daily_locked() {
            return;
        }
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        let language = self.config.language.clone();
        let mode = mode.unwrap_or(self.config.mode);
        if online.dailies.as_ref().is_some_and(|(d, _)| *d == today()) {
            self.pick_daily(&language, mode);
            return;
        }
        self.pending_daily = Some((language, mode));
        online.request(Request::DailiesToday);
        self.notify("loading daily…");
    }

    /// Find the listed daily for `(language, mode)` and fetch its words.
    fn pick_daily(&mut self, language: &str, mode: Mode) {
        let Some(online) = &mut self.online else {
            return;
        };
        let list = online
            .dailies
            .as_ref()
            .map(|(_, l)| l.as_slice())
            .unwrap_or(&[]);
        match list
            .iter()
            .find(|d| d.language == language && d.mode == mode)
        {
            Some(d) => {
                let id = d.id;
                online.request(Request::Daily(id));
                self.notify("loading daily…");
            }
            None => {
                let mut modes: Vec<String> = list
                    .iter()
                    .filter(|d| d.language == language)
                    .map(|d| d.mode.label())
                    .collect();
                if modes.is_empty() {
                    self.notify(format!("no daily for {language} today"));
                } else {
                    modes.sort();
                    self.notify(format!(
                        "no daily for {language} · {mode}; today: {}",
                        modes.join(", ")
                    ));
                }
            }
        }
    }

    /// Type the daily: its words in order, mode from the server, and every
    /// key recorded for the server to replay.
    fn start_daily(&mut self, daily: Daily) {
        let mut engine = TestEngine::new(
            daily.mode,
            Box::new(FixedGenerator::new(daily.words.clone())),
        );
        engine.record_keys();
        self.engine = engine;
        self.outcome = None;
        self.daily_start = DailyStart::None;
        let logged_in = self.online.as_ref().is_some_and(|o| o.logged_in());
        self.notify(format!(
            "daily · {} · {} · {}{}",
            daily.mode.label(),
            daily.language,
            daily.date,
            if logged_in {
                ""
            } else {
                " · not logged in, so this can't count as a first try"
            }
        ));
        self.daily = Some(daily);
        self.screen = Screen::Typing;
        self.previous_screen = Screen::Typing;
        self.scroll = 0;
    }

    /// Send the finished daily's keylog; queue it if that can't happen now.
    /// The first key of a daily was typed: tell the server, which counts
    /// it as an attempt whether or not the run is finished.
    pub(super) fn daily_began(&mut self) {
        let Some(daily) = &self.daily else {
            return;
        };
        self.daily_start = match &mut self.online {
            Some(o) if o.logged_in() => {
                o.request(Request::Start(daily.id));
                DailyStart::Pending { daily_id: daily.id }
            }
            _ => DailyStart::None,
        };
    }

    pub(super) fn submit_daily(&mut self, daily: &Daily) -> DailyOutcome {
        let start_id = match self.daily_start {
            DailyStart::Started { daily_id, start_id } if daily_id == daily.id => Some(start_id),
            _ => None,
        };
        let body = SubmitRequest {
            daily_id: daily.id,
            keylog: self.engine.keylog().map(<[_]>::to_vec).unwrap_or_default(),
            start_id,
        };
        // Finished before the server confirmed the start: send once it does.
        if self.daily_start == (DailyStart::Pending { daily_id: daily.id }) {
            self.held_submission = Some((body, daily.date.clone()));
            return DailyOutcome {
                daily_id: daily.id,
                status: DailyStatus::Submitting,
            };
        }
        let status = self.send_submission(body, &daily.date);
        DailyOutcome {
            daily_id: daily.id,
            status,
        }
    }

    fn send_submission(&mut self, body: SubmitRequest, date: &str) -> DailyStatus {
        match &mut self.online {
            Some(o) if o.logged_in() => {
                o.request(Request::Submit {
                    body,
                    date: date.to_string(),
                    queued: None,
                });
                DailyStatus::Submitting
            }
            Some(_) => self.queue_submission(date, body, "not logged in (:login)"),
            None => DailyStatus::Failed("offline".into()),
        }
    }

    /// The server's answer to a start: remember it for the submission, and
    /// send a submission that was waiting on it.
    fn started(
        &mut self,
        daily_id: i64,
        result: Result<ttyp_core::api::StartResponse, OnlineError>,
    ) {
        let current = self.daily_start == (DailyStart::Pending { daily_id });
        let start_id = match result {
            Ok(r) => {
                if current {
                    self.daily_start = DailyStart::Started {
                        daily_id,
                        start_id: r.start_id,
                    };
                }
                Some(r.start_id)
            }
            Err(e) => {
                if current {
                    self.daily_start = DailyStart::None;
                }
                self.notify(format!(
                    "daily start not recorded ({e}): this run can't be a first try"
                ));
                None
            }
        };
        if let Some((mut body, date)) = self
            .held_submission
            .take_if(|(b, _)| b.daily_id == daily_id)
        {
            body.start_id = start_id;
            let status = self.send_submission(body, &date);
            if let Some(d) = self
                .outcome
                .as_mut()
                .and_then(|o| o.daily.as_mut())
                .filter(|d| d.daily_id == daily_id)
            {
                d.status = status;
            }
        }
    }

    fn queue_submission(&mut self, date: &str, body: SubmitRequest, why: &str) -> DailyStatus {
        let q = queue::Queued {
            date: date.to_string(),
            request: body,
        };
        match queue::save(&self.paths.queue_dir, &q) {
            Ok(_) => {
                self.notify(format!("{why}: daily result queued for next start"));
                DailyStatus::Queued
            }
            Err(e) => DailyStatus::Failed(format!("{why}; could not queue: {e}")),
        }
    }

    fn submitted(
        &mut self,
        body: SubmitRequest,
        date: String,
        queued: Option<&Path>,
        result: Result<ttyp_core::api::SubmitResponse, OnlineError>,
    ) {
        let status = match result {
            Ok(r) => {
                if let Some(path) = queued {
                    let _ = queue::remove(path);
                    self.notify(match (r.valid, r.rank_best) {
                        (true, Some(n)) => format!("queued daily submitted: #{n} best"),
                        (true, None) => "queued daily submitted".to_string(),
                        (false, _) => "queued daily was rejected".to_string(),
                    });
                }
                DailyStatus::Ranked(r)
            }
            // A retry that failed again just stays queued.
            Err(_) if queued.is_some() => return,
            // The server said no; trying again later won't help.
            Err(OnlineError::Server { message, .. }) => DailyStatus::Failed(message),
            Err(e) => self.queue_submission(&date, body.clone(), &e.to_string()),
        };
        if let Some(d) = self
            .outcome
            .as_mut()
            .and_then(|o| o.daily.as_mut())
            .filter(|d| d.daily_id == body.daily_id)
        {
            d.status = status;
        }
    }

    /// On start: resend what couldn't be sent, drop what's too old.
    pub(super) fn retry_queued_submissions(&mut self) {
        let Some(online) = &mut self.online else {
            return;
        };
        let today = today();
        for (path, q) in queue::load(&self.paths.queue_dir) {
            if q.date != today {
                let _ = queue::remove(&path);
            } else if online.logged_in() {
                online.request(Request::Submit {
                    body: q.request,
                    date: q.date,
                    queued: Some(path),
                });
            }
        }
    }

    fn leave_login(&mut self) {
        self.login_prompt = None;
        if self.screen == Screen::Login {
            self.dispatch(super::Action::Back);
        }
    }
}
