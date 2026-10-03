//! The leaderboards, as data. The server reads `BOARDS` to know what each
//! board ranks, the client to know which boards exist, how they're grouped
//! into pages and which columns each shows. Adding a board is one entry in
//! `BOARDS` (the server needs a rebuild to serve it): pick a `Period` (which
//! runs it draws from), a `Board` rule (which run per player) and columns.

use serde::{Deserialize, Serialize};

use crate::api::Board;
use crate::test::Mode;

/// Which runs a board draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    /// One daily: one date, language and mode.
    Daily,
    /// Every daily ever held for one language and mode.
    AllTime,
}

impl Period {
    pub fn label(self) -> &'static str {
        match self {
            Period::Daily => "daily",
            Period::AllTime => "all time",
        }
    }
}

/// A column a board can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Rank,
    User,
    Wpm,
    Acc,
    Consistency,
    /// The UTC date of the daily the run was on.
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardSpec {
    /// Stable name on the wire: `GET /boards/{id}`.
    pub id: &'static str,
    pub label: &'static str,
    pub period: Period,
    /// Which of a player's runs is ranked.
    pub rule: Board,
    pub columns: &'static [Stat],
}

const DAILY_COLUMNS: &[Stat] = &[
    Stat::Rank,
    Stat::User,
    Stat::Wpm,
    Stat::Acc,
    Stat::Consistency,
];
const ALL_TIME_COLUMNS: &[Stat] = &[Stat::Rank, Stat::User, Stat::Wpm, Stat::Acc, Stat::Date];

/// Every board, in display order: pages follow the order periods first
/// appear in, and boards keep their order within a page.
pub const BOARDS: &[BoardSpec] = &[
    BoardSpec {
        id: "daily-first",
        label: "first try",
        period: Period::Daily,
        rule: Board::First,
        columns: DAILY_COLUMNS,
    },
    BoardSpec {
        id: "daily-best",
        label: "best",
        period: Period::Daily,
        rule: Board::Best,
        columns: DAILY_COLUMNS,
    },
    BoardSpec {
        id: "alltime-first",
        label: "best first try",
        period: Period::AllTime,
        rule: Board::First,
        columns: ALL_TIME_COLUMNS,
    },
    BoardSpec {
        id: "alltime-best",
        label: "best",
        period: Period::AllTime,
        rule: Board::Best,
        columns: ALL_TIME_COLUMNS,
    },
];

/// What one board ranks within its period: a daily, or a language and
/// mode across every daily.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Daily(i64),
    Mode { language: String, mode: Mode },
}

impl Target {
    /// The query string of `GET /boards/{id}` that names this target.
    pub fn query(&self) -> String {
        match self {
            Target::Daily(id) => format!("daily={id}"),
            Target::Mode { language, mode } => format!(
                "language={language}&kind={}&value={}",
                mode.kind(),
                mode.value()
            ),
        }
    }
}

pub fn find(id: &str) -> Option<&'static BoardSpec> {
    BOARDS.iter().find(|b| b.id == id)
}

/// The periods that have boards, in the order they first appear.
pub fn periods() -> Vec<Period> {
    let mut v: Vec<Period> = Vec::new();
    for b in BOARDS {
        if !v.contains(&b.period) {
            v.push(b.period);
        }
    }
    v
}

/// The boards of one period, in order.
pub fn on(period: Period) -> Vec<&'static BoardSpec> {
    BOARDS.iter().filter(|b| b.period == period).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_pages_follow_order() {
        for (i, b) in BOARDS.iter().enumerate() {
            assert!(
                BOARDS[..i].iter().all(|o| o.id != b.id),
                "duplicate id {}",
                b.id
            );
            assert_eq!(find(b.id), Some(b));
        }
        assert_eq!(periods(), [Period::Daily, Period::AllTime]);
        let ids: Vec<&str> = on(Period::AllTime).iter().map(|b| b.id).collect();
        assert_eq!(ids, ["alltime-first", "alltime-best"]);
        assert_eq!(find("nope"), None);
    }
}
