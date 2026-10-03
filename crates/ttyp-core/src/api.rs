//! Request and response bodies shared by the client and the server. Field
//! names are the JSON names; keep them stable.

use serde::{Deserialize, Serialize};

use crate::test::keylog::KeyEvent;
use crate::test::metrics::CharCounts;
use crate::test::mode::Mode;

/// The modes the server schedules dailies for (see the `daily_schedule`
/// seed); the client offers these for `:daily <mode>`.
pub const DAILY_MODES: [Mode; 7] = [
    Mode::Time(15),
    Mode::Time(30),
    Mode::Time(60),
    Mode::Words(10),
    Mode::Words(25),
    Mode::Words(50),
    Mode::Words(100),
];

/// One of today's dailies, as listed by `GET /dailies/today`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailySummary {
    pub id: i64,
    /// UTC date, `YYYY-MM-DD`.
    pub date: String,
    pub language: String,
    pub mode: Mode,
}

/// A daily with its words: `GET /dailies/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Daily {
    pub id: i64,
    pub date: String,
    pub language: String,
    pub mode: Mode,
    pub words: Vec<String>,
}

/// `POST /results`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitRequest {
    pub daily_id: i64,
    pub keylog: Vec<KeyEvent>,
    /// From `POST /dailies/{id}/start`, sent when the run began. A run
    /// without one still counts as an attempt but can't be a first try.
    /// Absent from clients before 1.2.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_id: Option<i64>,
}

/// `POST /dailies/{id}/start`: the client typed the first key of a daily.
/// Every start uses up an attempt, whether or not a result follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartResponse {
    pub start_id: i64,
    pub attempt: u32,
}

/// What the server made of a submission. Ranks are 1-based and only
/// present for valid runs; `rank_first` only when this was the first try.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitResponse {
    pub result_id: i64,
    pub attempt: u32,
    pub valid: bool,
    /// Why the run was rejected, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected: Option<String>,
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub rank_first: Option<u32>,
    pub rank_best: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Board {
    First,
    Best,
}

impl Board {
    pub fn label(self) -> &'static str {
        match self {
            Board::First => "first try",
            Board::Best => "best",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardRow {
    pub rank: u32,
    pub user: String,
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub result_id: i64,
    /// UTC date of the daily the run was on. Absent from servers before 2.1.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// `GET /leaderboard/{daily_id}` and `GET /boards/{id}` (`boards::BOARDS`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Leaderboard {
    pub board: Board,
    /// Rows from `offset` on, in rank order.
    pub rows: Vec<LeaderboardRow>,
    pub offset: u32,
    pub total: u32,
    /// The caller's own row, when logged in and on the board.
    pub me: Option<LeaderboardRow>,
}

/// `GET /results/{id}`: everything the graph view shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultDetail {
    pub id: i64,
    pub daily: DailySummary,
    pub user: String,
    pub attempt: u32,
    pub wpm: f64,
    pub raw: f64,
    pub acc: f64,
    pub consistency: f64,
    pub duration_s: f64,
    pub chars: CharCounts,
    pub wpm_per_second: Vec<f64>,
    pub raw_per_second: Vec<f64>,
    pub errors_per_second: Vec<u32>,
}

/// `GET /account` and `POST /account`: the caller's own account settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    /// Whether `GET /users/{login}` shows this profile to everyone.
    pub public: bool,
}

/// `POST /account`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountUpdate {
    pub public: bool,
}

/// `GET /users/{login}`: a public profile, or your own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub login: String,
    pub public: bool,
    /// UTC date the account was created (`YYYY-MM-DD`).
    pub joined: String,
    /// Consecutive UTC days, up to today or yesterday, with a valid daily.
    pub streak: u32,
    /// Dailies with at least one valid run.
    pub dailies: u32,
    /// Best valid run per language and mode.
    pub bests: Vec<ProfileRun>,
    /// Latest valid runs, newest first.
    pub recent: Vec<ProfileRun>,
    /// Top-three finishes on first-try boards of finished dailies.
    /// Absent from servers before 1.3.0.
    #[serde(default)]
    pub badges: Badges,
}

/// The main dailies: medals are only awarded on these.
pub const MEDAL_DAILIES: [(&str, Mode); 3] = [
    ("english", Mode::Time(15)),
    ("english", Mode::Time(30)),
    ("english", Mode::Time(60)),
];

/// A player's top-three finishes on first-try boards of finished dailies:
/// gold, silver and bronze on the main dailies (`MEDAL_DAILIES`), and
/// every other daily's top-three finishes lumped together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Badges {
    pub gold: u32,
    pub silver: u32,
    pub bronze: u32,
    /// Top-three finishes on any other daily.
    pub other: u32,
}

impl Badges {
    pub fn medals(&self) -> u32 {
        self.gold + self.silver + self.bronze
    }
}

/// One run on a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRun {
    pub result_id: i64,
    pub date: String,
    pub language: String,
    pub mode: Mode,
    pub wpm: f64,
    pub acc: f64,
    pub attempt: u32,
}

/// `POST /auth/github`: trade a GitHub access token for a ttyp token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthRequest {
    pub access_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub login: String,
}

/// Every error response: `{"error": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}
