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
}

/// `GET /leaderboard/{daily_id}`.
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
