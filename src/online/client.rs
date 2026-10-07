//! Blocking HTTP client for the ttyp server API (`ttyp_core::api`).

use std::fmt;
use std::time::Duration;

use serde::de::DeserializeOwned;
use ttyp_core::api::{
    Account, AccountUpdate, AuthRequest, AuthResponse, Daily, DailySummary, ErrorBody,
    FollowUpdate, Leaderboard, PlayerList, PlayerSummary, Profile, ResultDetail, StartResponse,
    SubmitRequest, SubmitResponse,
};
use ttyp_core::boards::Target;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnlineError {
    /// Couldn't reach the server (DNS, connection, timeout, bad URL).
    Unreachable(String),
    /// The token was missing or rejected.
    Unauthorized,
    /// The server answered with an error (`{"error": …}`).
    Server { status: u16, message: String },
}

impl fmt::Display for OnlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OnlineError::Unreachable(e) => write!(f, "server unreachable: {e}"),
            OnlineError::Unauthorized => write!(f, "not logged in (:login)"),
            OnlineError::Server { message, .. } => write!(f, "{message}"),
        }
    }
}

impl From<ureq::Error> for OnlineError {
    fn from(e: ureq::Error) -> Self {
        OnlineError::Unreachable(e.to_string())
    }
}

/// `s` percent-encoded for a query string.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Clone)]
pub struct Client {
    base: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(base: &str, token: Option<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(concat!("ttyp/", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Self {
            base: base.trim_end_matches('/').to_string(),
            token,
            agent,
        }
    }

    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Shared by every call: GET or POST, bearer token, JSON in and out,
    /// and `{"error": …}` bodies turned into `OnlineError::Server`.
    fn call<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&impl serde::Serialize>,
    ) -> Result<T, OnlineError> {
        let url = format!("{}{path}", self.base);
        let bearer = self.token.as_ref().map(|t| format!("Bearer {t}"));
        let mut res = match body {
            Some(b) => {
                let mut req = self.agent.post(&url);
                if let Some(b) = &bearer {
                    req = req.header("Authorization", b);
                }
                req.send_json(b)?
            }
            None => {
                let mut req = self.agent.get(&url);
                if let Some(b) = &bearer {
                    req = req.header("Authorization", b);
                }
                req.call()?
            }
        };
        let status = res.status().as_u16();
        if status == 401 {
            return Err(OnlineError::Unauthorized);
        }
        if status >= 400 {
            let message = res
                .body_mut()
                .read_json::<ErrorBody>()
                .map(|e| e.error)
                .unwrap_or_else(|_| format!("http {status}"));
            return Err(OnlineError::Server { status, message });
        }
        res.body_mut()
            .read_json()
            .map_err(|e| OnlineError::Unreachable(format!("bad response: {e}")))
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, OnlineError> {
        self.call(path, None::<&()>)
    }

    pub fn dailies_today(&self) -> Result<Vec<DailySummary>, OnlineError> {
        self.get("/dailies/today")
    }

    /// The dailies of a UTC day (`YYYY-MM-DD`).
    pub fn dailies_for(&self, date: &str) -> Result<Vec<DailySummary>, OnlineError> {
        self.get(&format!("/dailies?date={date}"))
    }

    pub fn daily(&self, id: i64) -> Result<Daily, OnlineError> {
        self.get(&format!("/dailies/{id}"))
    }

    /// Tell the server a daily run began; it counts as an attempt.
    pub fn start(&self, daily_id: i64) -> Result<StartResponse, OnlineError> {
        self.call(&format!("/dailies/{daily_id}/start"), Some(&()))
    }

    pub fn submit(&self, req: &SubmitRequest) -> Result<SubmitResponse, OnlineError> {
        self.call("/results", Some(req))
    }

    /// A page of one board of `boards::BOARDS`; `following` ranks only us
    /// and who we follow.
    pub fn board(
        &self,
        id: &str,
        target: &Target,
        following: bool,
        offset: u32,
        limit: u32,
    ) -> Result<Leaderboard, OnlineError> {
        let following = if following { "&following=true" } else { "" };
        self.get(&format!(
            "/boards/{id}?{}{following}&offset={offset}&limit={limit}",
            target.query()
        ))
    }

    pub fn result(&self, id: i64) -> Result<ResultDetail, OnlineError> {
        self.get(&format!("/results/{id}"))
    }

    pub fn account(&self) -> Result<Account, OnlineError> {
        self.get("/account")
    }

    pub fn set_public(&self, public: bool) -> Result<Account, OnlineError> {
        self.call("/account", Some(&AccountUpdate { public }))
    }

    /// A public profile, or your own. Logins are GitHub names, which only
    /// use letters, digits and `-`, so they go into the path as they are.
    pub fn user(&self, login: &str) -> Result<Profile, OnlineError> {
        self.get(&format!("/users/{login}"))
    }

    /// A page of public players whose login contains `query`.
    pub fn players(&self, query: &str, offset: u32, limit: u32) -> Result<PlayerList, OnlineError> {
        self.get(&format!(
            "/users?q={}&offset={offset}&limit={limit}",
            encode(query)
        ))
    }

    /// Who we follow.
    pub fn follows(&self) -> Result<Vec<PlayerSummary>, OnlineError> {
        self.get("/follows")
    }

    /// Follow or unfollow `login`; answers with the new follow list.
    pub fn set_follow(&self, login: &str, follow: bool) -> Result<Vec<PlayerSummary>, OnlineError> {
        self.call(
            "/follows",
            Some(&FollowUpdate {
                login: login.to_string(),
                follow,
            }),
        )
    }

    /// Trade a GitHub access token for a ttyp token.
    pub fn auth_github(&self, access_token: &str) -> Result<AuthResponse, OnlineError> {
        self.call(
            "/auth/github",
            Some(&AuthRequest {
                access_token: access_token.to_string(),
            }),
        )
    }

    /// Revoke the current token on the server.
    pub fn logout(&self) -> Result<(), OnlineError> {
        self.call::<serde_json::Value>("/auth/logout", Some(&()))
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn queries_are_percent_encoded() {
        assert_eq!(super::encode("ann-1_x"), "ann-1_x");
        assert_eq!(super::encode("a b&c=%é"), "a%20b%26c%3D%25%C3%A9");
    }
}
