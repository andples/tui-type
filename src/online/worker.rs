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
    Account, CustomList, CustomPublish, CustomSet, CustomSummary, Daily, DailySummary, Leaderboard,
    PlayerList, PlayerSummary, Profile, ResultDetail, StartResponse, SubmitRequest, SubmitResponse,
};

use ttyp_core::boards::Target;
use ttyp_core::sync::{Best, ConfigPush, ConfigSync};

use super::client::{Client, OnlineError};
use super::device;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Login,
    Logout,
    DailiesToday,
    /// The dailies of another UTC day (leaderboard day switching).
    DailiesFor(String),
    Daily(i64),
    /// The first key of this daily was typed.
    Start(i64),
    Submit {
        body: SubmitRequest,
        /// UTC date of the daily, so a failed submission can be queued.
        date: String,
        /// The queue file this came from, if it's a retry.
        queued: Option<PathBuf>,
    },
    /// A page of one board (`boards::BoardSpec::id`).
    Leaderboard {
        board: &'static str,
        target: Target,
        /// Only us and who we follow.
        following: bool,
        offset: u32,
        limit: u32,
    },
    Result(i64),
    Account,
    SetPublic(bool),
    User(String),
    /// A page of the player search.
    Players {
        query: String,
        offset: u32,
        limit: u32,
    },
    /// Who we follow.
    Follows,
    /// Follow (`true`) or unfollow someone.
    SetFollow {
        login: String,
        follow: bool,
    },
    /// A page of published custom sets.
    CustomList {
        query: String,
        offset: u32,
        limit: u32,
    },
    CustomInstall(String),
    CustomPublish(CustomPublish),
    CustomUnpublish(String),
    /// The account's synced configs.
    Configs,
    /// Replace the account's configs.
    PushConfigs(ConfigPush),
    /// Send bests; the reply is all of them.
    PushBests(Vec<Best>),
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
    DailiesFor {
        date: String,
        result: Result<Vec<DailySummary>, OnlineError>,
    },
    Daily(Result<Daily, OnlineError>),
    Started {
        daily_id: i64,
        result: Result<StartResponse, OnlineError>,
    },
    Submitted {
        body: SubmitRequest,
        date: String,
        queued: Option<PathBuf>,
        result: Result<SubmitResponse, OnlineError>,
    },
    Leaderboard {
        board: &'static str,
        target: Target,
        following: bool,
        offset: u32,
        result: Result<Leaderboard, OnlineError>,
    },
    ResultDetail(Result<ResultDetail, OnlineError>),
    Account(Result<Account, OnlineError>),
    User {
        login: String,
        result: Result<Profile, OnlineError>,
    },
    Players {
        query: String,
        offset: u32,
        result: Result<PlayerList, OnlineError>,
    },
    Follows(Result<Vec<PlayerSummary>, OnlineError>),
    /// A follow change went through (the new list) or didn't.
    Followed {
        login: String,
        follow: bool,
        result: Result<Vec<PlayerSummary>, OnlineError>,
    },
    CustomList {
        query: String,
        result: Result<CustomList, OnlineError>,
    },
    CustomInstalled {
        name: String,
        result: Result<CustomSet, OnlineError>,
    },
    CustomPublished {
        name: String,
        result: Result<CustomSummary, OnlineError>,
    },
    CustomUnpublished {
        name: String,
        result: Result<(), OnlineError>,
    },
    /// The account's configs, fetched (`pushed` is `None`) or as they are
    /// after a push.
    Configs {
        pushed: Option<ConfigPush>,
        result: Result<ConfigSync, OnlineError>,
    },
    Bests(Result<Vec<Best>, OnlineError>),
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
                spawn(
                    tx,
                    |e| RemoteEvent::LoginFailed(e.to_string()),
                    move |tx| login(&client, &id, tx, &cancel),
                );
            }
            Request::Logout => spawn(
                tx,
                |_| RemoteEvent::LoggedOut,
                move |_| {
                    // Best effort: the local token goes either way.
                    let _ = client.logout();
                    RemoteEvent::LoggedOut
                },
            ),
            Request::DailiesToday => spawn(
                tx,
                |e| RemoteEvent::Dailies(Err(e)),
                move |_| RemoteEvent::Dailies(client.dailies_today()),
            ),
            Request::DailiesFor(date) => {
                let d = date.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::DailiesFor {
                        date: d,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.dailies_for(&date);
                        RemoteEvent::DailiesFor { date, result }
                    },
                );
            }
            Request::Daily(id) => spawn(
                tx,
                |e| RemoteEvent::Daily(Err(e)),
                move |_| RemoteEvent::Daily(client.daily(id)),
            ),
            Request::Start(daily_id) => spawn(
                tx,
                move |e| RemoteEvent::Started {
                    daily_id,
                    result: Err(e),
                },
                move |_| RemoteEvent::Started {
                    daily_id,
                    result: client.start(daily_id),
                },
            ),
            Request::Submit { body, date, queued } => {
                let (b, d, q) = (body.clone(), date.clone(), queued.clone());
                spawn(
                    tx,
                    |e| RemoteEvent::Submitted {
                        body: b,
                        date: d,
                        queued: q,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.submit(&body);
                        RemoteEvent::Submitted {
                            body,
                            date,
                            queued,
                            result,
                        }
                    },
                );
            }
            Request::Leaderboard {
                board,
                target,
                following,
                offset,
                limit,
            } => {
                let t = target.clone();
                spawn(
                    tx,
                    move |e| RemoteEvent::Leaderboard {
                        board,
                        target: t,
                        following,
                        offset,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.board(board, &target, following, offset, limit);
                        RemoteEvent::Leaderboard {
                            board,
                            target,
                            following,
                            offset,
                            result,
                        }
                    },
                );
            }
            Request::Result(id) => spawn(
                tx,
                |e| RemoteEvent::ResultDetail(Err(e)),
                move |_| RemoteEvent::ResultDetail(client.result(id)),
            ),
            Request::Account => spawn(
                tx,
                |e| RemoteEvent::Account(Err(e)),
                move |_| RemoteEvent::Account(client.account()),
            ),
            Request::SetPublic(public) => spawn(
                tx,
                |e| RemoteEvent::Account(Err(e)),
                move |_| RemoteEvent::Account(client.set_public(public)),
            ),
            Request::User(login) => {
                let l = login.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::User {
                        login: l,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.user(&login);
                        RemoteEvent::User { login, result }
                    },
                );
            }
            Request::Players {
                query,
                offset,
                limit,
            } => {
                let q = query.clone();
                spawn(
                    tx,
                    move |e| RemoteEvent::Players {
                        query: q,
                        offset,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.players(&query, offset, limit);
                        RemoteEvent::Players {
                            query,
                            offset,
                            result,
                        }
                    },
                );
            }
            Request::Follows => spawn(
                tx,
                |e| RemoteEvent::Follows(Err(e)),
                move |_| RemoteEvent::Follows(client.follows()),
            ),
            Request::CustomList {
                query,
                offset,
                limit,
            } => {
                let q = query.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::CustomList {
                        query: q,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.custom_list(&query, offset, limit);
                        RemoteEvent::CustomList { query, result }
                    },
                );
            }
            Request::CustomInstall(name) => {
                let n = name.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::CustomInstalled {
                        name: n,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.custom_install(&name);
                        RemoteEvent::CustomInstalled { name, result }
                    },
                );
            }
            Request::CustomPublish(set) => {
                let n = set.name.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::CustomPublished {
                        name: n,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.custom_publish(&set);
                        RemoteEvent::CustomPublished {
                            name: set.name,
                            result,
                        }
                    },
                );
            }
            Request::CustomUnpublish(name) => {
                let n = name.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::CustomUnpublished {
                        name: n,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.custom_unpublish(&name);
                        RemoteEvent::CustomUnpublished { name, result }
                    },
                );
            }
            Request::Configs => spawn(
                tx,
                |e| RemoteEvent::Configs {
                    pushed: None,
                    result: Err(e),
                },
                move |_| RemoteEvent::Configs {
                    pushed: None,
                    result: client.configs(),
                },
            ),
            Request::PushConfigs(push) => {
                let p = push.clone();
                spawn(
                    tx,
                    |e| RemoteEvent::Configs {
                        pushed: Some(p),
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.push_configs(&push);
                        RemoteEvent::Configs {
                            pushed: Some(push),
                            result,
                        }
                    },
                );
            }
            Request::PushBests(bests) => spawn(
                tx,
                |e| RemoteEvent::Bests(Err(e)),
                move |_| RemoteEvent::Bests(client.push_bests(&bests)),
            ),
            Request::SetFollow { login, follow } => {
                let l = login.clone();
                spawn(
                    tx,
                    move |e| RemoteEvent::Followed {
                        login: l,
                        follow,
                        result: Err(e),
                    },
                    move |_| {
                        let result = client.set_follow(&login, follow);
                        RemoteEvent::Followed {
                            login,
                            follow,
                            result,
                        }
                    },
                );
            }
        }
    }
}

/// Run `work` on its own thread and send what it returns. If it panics,
/// `failed` makes the reply instead (with `OnlineError::Internal`), so the
/// request still ends and `pending` drops back.
fn spawn(
    tx: Sender<RemoteEvent>,
    failed: impl FnOnce(OnlineError) -> RemoteEvent + Send + 'static,
    work: impl FnOnce(&Sender<RemoteEvent>) -> RemoteEvent + Send + 'static,
) {
    thread::spawn(move || {
        let ev = crate::errlog::catch_panic("online", || work(&tx))
            .unwrap_or_else(|msg| failed(OnlineError::Internal(msg)));
        let _ = tx.send(ev);
    });
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
