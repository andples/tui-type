//! Background network work. Each `Request` runs on its own thread and
//! reports back as `RemoteEvent`s over a channel that the event loop
//! drains; rendering never waits on the network.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use ttyp_core::api::{
    Board, Daily, DailySummary, Leaderboard, ResultDetail, SubmitRequest, SubmitResponse,
};

use super::client::{Client, OnlineError};
use super::device;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Login,
    Logout,
    DailiesToday,
    Daily(i64),
    Submit {
        body: SubmitRequest,
        /// UTC date of the daily, so a failed submission can be queued.
        date: String,
        /// The queue file this came from, if it's a retry.
        queued: Option<PathBuf>,
    },
    Leaderboard {
        daily_id: i64,
        board: Board,
        offset: u32,
        limit: u32,
    },
    Result(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteEvent {
    /// Show this to the user; the flow keeps polling.
    DeviceCode {
        user_code: String,
        verification_uri: String,
    },
    LoggedIn {
        login: String,
        token: String,
    },
    LoginFailed(String),
    LoggedOut,
    Dailies(Result<Vec<DailySummary>, OnlineError>),
    Daily(Result<Daily, OnlineError>),
    Submitted {
        body: SubmitRequest,
        date: String,
        queued: Option<PathBuf>,
        result: Result<SubmitResponse, OnlineError>,
    },
    Leaderboard {
        daily_id: i64,
        board: Board,
        offset: u32,
        result: Result<Leaderboard, OnlineError>,
    },
    ResultDetail(Result<ResultDetail, OnlineError>),
}

impl RemoteEvent {
    /// Whether this event ends the request that produced it.
    fn is_final(&self) -> bool {
        !matches!(self, RemoteEvent::DeviceCode { .. })
    }
}

/// The client's online state: the server client, the GitHub app id and the
/// channel of replies from worker threads.
pub struct Online {
    pub client: Client,
    pub github_client_id: Option<String>,
    /// Today's dailies, keyed by the UTC date they were fetched for.
    pub dailies: Option<(String, Vec<DailySummary>)>,
    tx: Sender<RemoteEvent>,
    rx: Receiver<RemoteEvent>,
    /// Requests started and not yet answered.
    pending: usize,
    cancel_login: Arc<AtomicBool>,
}

impl Online {
    pub fn new(client: Client, github_client_id: Option<String>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            client,
            github_client_id,
            dailies: None,
            tx,
            rx,
            pending: 0,
            cancel_login: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn logged_in(&self) -> bool {
        self.client.token().is_some()
    }

    /// Something is in flight, so the event loop should poll the channel.
    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// Replies that have arrived since the last call.
    pub fn poll(&mut self) -> Vec<RemoteEvent> {
        let events: Vec<RemoteEvent> = self.rx.try_iter().collect();
        self.pending = self
            .pending
            .saturating_sub(events.iter().filter(|e| e.is_final()).count());
        events
    }

    pub fn cancel_login(&self) {
        self.cancel_login.store(true, Ordering::Relaxed);
    }

    /// Start `req` on a background thread.
    pub fn request(&mut self, req: Request) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        self.pending += 1;
        match req {
            Request::Login => {
                let cancel = Arc::clone(&self.cancel_login);
                cancel.store(false, Ordering::Relaxed);
                let id = self.github_client_id.clone().unwrap_or_default();
                thread::spawn(move || {
                    let _ = tx.send(login(&client, &id, &tx, &cancel));
                });
            }
            Request::Logout => {
                thread::spawn(move || {
                    // Best effort: the local token goes either way.
                    let _ = client.logout();
                    let _ = tx.send(RemoteEvent::LoggedOut);
                });
            }
            Request::DailiesToday => {
                thread::spawn(move || {
                    let _ = tx.send(RemoteEvent::Dailies(client.dailies_today()));
                });
            }
            Request::Daily(id) => {
                thread::spawn(move || {
                    let _ = tx.send(RemoteEvent::Daily(client.daily(id)));
                });
            }
            Request::Submit { body, date, queued } => {
                thread::spawn(move || {
                    let result = client.submit(&body);
                    let _ = tx.send(RemoteEvent::Submitted {
                        body,
                        date,
                        queued,
                        result,
                    });
                });
            }
            Request::Leaderboard {
                daily_id,
                board,
                offset,
                limit,
            } => {
                thread::spawn(move || {
                    let result = client.leaderboard(daily_id, board, offset, limit);
                    let _ = tx.send(RemoteEvent::Leaderboard {
                        daily_id,
                        board,
                        offset,
                        result,
                    });
                });
            }
            Request::Result(id) => {
                thread::spawn(move || {
                    let _ = tx.send(RemoteEvent::ResultDetail(client.result(id)));
                });
            }
        }
    }
}

/// Device flow against GitHub, then the token exchange with the server.
fn login(
    client: &Client,
    client_id: &str,
    tx: &Sender<RemoteEvent>,
    cancel: &AtomicBool,
) -> RemoteEvent {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .new_agent();
    let github_token = match device::run(
        &agent,
        client_id,
        |code| {
            let _ = tx.send(RemoteEvent::DeviceCode {
                user_code: code.user_code.clone(),
                verification_uri: code.verification_uri.clone(),
            });
        },
        cancel,
        thread::sleep,
    ) {
        Ok(t) => t,
        Err(e) => return RemoteEvent::LoginFailed(e),
    };
    // The GitHub token is used once and dropped; only the ttyp token is kept.
    match client.auth_github(&github_token) {
        Ok(r) => RemoteEvent::LoggedIn {
            login: r.login,
            token: r.token,
        },
        Err(e) => RemoteEvent::LoginFailed(e.to_string()),
    }
}
