//! Leaderboard screen state, no rendering: which period is shown (one page
//! per `boards::periods()`), which daily or language and mode, the rows and
//! cursor of each of that period's boards, and which board has focus. The
//! boards themselves come from `ttyp_core::boards::BOARDS`. The app feeds it
//! server replies; the UI reads it.

use ttyp_core::api::{DailySummary, Leaderboard, LeaderboardRow};
use ttyp_core::boards::{self, BoardSpec, Period, Target};
use ttyp_core::test::Mode;

use crate::ui::widgets::Selection;

/// Rows fetched per request.
pub const PAGE: u32 = 50;
/// Fetch the next page when the cursor gets this close to the end.
const PREFETCH_MARGIN: usize = 10;

#[derive(Debug, Default)]
pub struct BoardPane {
    pub rows: Vec<LeaderboardRow>,
    pub total: u32,
    /// Your own row, when logged in and on the board.
    pub me: Option<LeaderboardRow>,
    pub selection: Selection,
    pub loading: bool,
    pub error: Option<String>,
}

impl BoardPane {
    pub fn reset(&mut self) {
        *self = BoardPane {
            loading: true,
            ..Default::default()
        };
    }

    /// Merge a reply: the first page replaces, later pages append.
    pub fn apply(&mut self, lb: Leaderboard) {
        self.loading = false;
        self.error = None;
        if lb.offset == 0 {
            self.rows = lb.rows;
        } else if lb.offset as usize == self.rows.len() {
            self.rows.extend(lb.rows);
        } else {
            return; // stale page
        }
        self.total = lb.total;
        self.me = lb.me;
        self.selection.set_len(self.rows.len());
    }

    pub fn fail(&mut self, e: String) {
        self.loading = false;
        self.error = Some(e);
    }

    /// Where your row sits in `rows`, if it's been fetched.
    pub fn my_index(&self) -> Option<usize> {
        let me = self.me.as_ref()?;
        self.rows.iter().position(|r| r.result_id == me.result_id)
    }

    /// The cursor is near the end and the server has more.
    pub fn wants_more(&self) -> bool {
        !self.loading
            && self.error.is_none()
            && (self.rows.len() as u32) < self.total
            && self.selection.near_end(PREFETCH_MARGIN)
    }

    pub fn selected(&self) -> Option<&LeaderboardRow> {
        self.rows.get(self.selection.selected)
    }
}

#[derive(Debug)]
pub struct BoardView {
    pub period: Period,
    pub date: String,
    pub language: String,
    pub mode: Mode,
    /// The dailies of `date`, once fetched. All-time boards offer the
    /// languages and modes of these too.
    pub dailies: Vec<DailySummary>,
    pub dailies_loading: bool,
    /// One per board of `period`, in `boards::on(period)` order.
    pub panes: Vec<BoardPane>,
    /// Index into `panes`.
    pub focus: usize,
    pub error: Option<String>,
    /// Rank only us and the players we follow.
    pub following: bool,
}

/// Time modes first, then words, each ascending.
fn mode_key(m: &Mode) -> (u8, u16) {
    match m {
        Mode::Time(v) => (0, *v),
        Mode::Words(v) => (1, *v),
    }
}

impl BoardView {
    pub fn new(date: String, language: String, mode: Mode) -> Self {
        let period = boards::periods()[0];
        Self {
            period,
            date,
            language,
            mode,
            dailies: Vec::new(),
            dailies_loading: true,
            panes: Self::panes_for(period),
            focus: 0,
            error: None,
            following: false,
        }
    }

    fn panes_for(period: Period) -> Vec<BoardPane> {
        boards::on(period)
            .iter()
            .map(|_| BoardPane::default())
            .collect()
    }

    /// The boards on screen, parallel to `panes`.
    pub fn boards(&self) -> Vec<&'static BoardSpec> {
        boards::on(self.period)
    }

    /// Show the next period's boards, wrapping, focusing the first.
    pub fn cycle_period(&mut self) {
        let periods = boards::periods();
        let i = periods.iter().position(|p| *p == self.period).unwrap_or(0);
        self.period = periods[(i + 1) % periods.len()];
        self.panes = Self::panes_for(self.period);
        self.focus = 0;
    }

    /// What the boards rank: the shown daily, or the language and mode
    /// across all dailies. `None` while there's nothing to show.
    pub fn target(&self) -> Option<Target> {
        let daily = self.daily()?;
        Some(match self.period {
            Period::Daily => Target::Daily(daily.id),
            Period::AllTime => Target::Mode {
                language: daily.language.clone(),
                mode: daily.mode,
            },
        })
    }

    /// The daily currently shown, if the day has one for the selection.
    pub fn daily(&self) -> Option<&DailySummary> {
        self.dailies
            .iter()
            .find(|d| d.language == self.language && d.mode == self.mode)
    }

    /// The day's dailies arrived. If the current language/mode has none,
    /// fall back to the same language's first mode, then to anything.
    pub fn set_dailies(&mut self, list: Vec<DailySummary>) {
        self.dailies = list;
        self.dailies_loading = false;
        if self.daily().is_some() {
            return;
        }
        let mut modes = self.modes_for(&self.language);
        if modes.is_empty()
            && let Some(first) = self.languages().first()
        {
            self.language = first.clone();
            modes = self.modes_for(&self.language);
        }
        if let Some(m) = modes.first() {
            self.mode = *m;
        }
    }

    /// Languages with a daily on this day, sorted.
    pub fn languages(&self) -> Vec<String> {
        let mut v: Vec<String> = self.dailies.iter().map(|d| d.language.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Modes with a daily for `language` on this day, in menu order.
    pub fn modes_for(&self, language: &str) -> Vec<Mode> {
        let mut v: Vec<Mode> = self
            .dailies
            .iter()
            .filter(|d| d.language == language)
            .map(|d| d.mode)
            .collect();
        v.sort_by_key(mode_key);
        v.dedup();
        v
    }

    /// The pane of the board with this id, if it's on screen.
    pub fn pane_mut(&mut self, id: &str) -> Option<&mut BoardPane> {
        let i = self.boards().iter().position(|b| b.id == id)?;
        self.panes.get_mut(i)
    }

    pub fn focused(&mut self) -> &mut BoardPane {
        &mut self.panes[self.focus]
    }

    pub fn focused_board(&self) -> &'static BoardSpec {
        self.boards()[self.focus]
    }

    /// Focus the next board, wrapping.
    pub fn next_focus(&mut self) {
        self.focus = (self.focus + 1) % self.panes.len().max(1);
    }

    /// Forget both boards' rows (before a reload).
    pub fn reset_panes(&mut self) {
        for p in &mut self.panes {
            p.reset();
        }
    }

    /// Step to the next/previous mode of this language, wrapping. `false`
    /// when there's nothing to step to.
    pub fn cycle_mode(&mut self, dir: isize) -> bool {
        let modes = self.modes_for(&self.language);
        Self::step(&modes, &mut self.mode, dir)
    }

    pub fn cycle_language(&mut self, dir: isize) -> bool {
        let langs = self.languages();
        if !Self::step(&langs, &mut self.language, dir) {
            return false;
        }
        // Keep the mode if the new language has it, else its first.
        let modes = self.modes_for(&self.language);
        if !modes.contains(&self.mode)
            && let Some(m) = modes.first()
        {
            self.mode = *m;
        }
        true
    }

    fn step<T: Clone + PartialEq>(list: &[T], current: &mut T, dir: isize) -> bool {
        if list.len() < 2 {
            return false;
        }
        let i = list.iter().position(|x| x == current).unwrap_or(0);
        let n = list.len() as isize;
        *current = list[(i as isize + dir).rem_euclid(n) as usize].clone();
        true
    }

    /// Move `dir` days; returns the new date. The caller fetches its dailies.
    pub fn shift_day(&mut self, dir: i64) -> String {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            let next = d + chrono::Duration::days(dir);
            self.date = next.to_string();
        }
        self.dailies.clear();
        self.dailies_loading = true;
        self.reset_panes();
        self.date.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttyp_core::api::Board;

    fn daily(id: i64, language: &str, mode: Mode) -> DailySummary {
        DailySummary {
            id,
            date: "2026-09-29".into(),
            language: language.into(),
            mode,
        }
    }

    fn row(rank: u32, user: &str) -> LeaderboardRow {
        LeaderboardRow {
            rank,
            user: user.into(),
            wpm: 100.0,
            raw: 105.0,
            acc: 97.0,
            consistency: 70.0,
            result_id: rank as i64 * 10,
            date: None,
        }
    }

    fn view() -> BoardView {
        let mut v = BoardView::new("2026-09-29".into(), "english".into(), Mode::Time(30));
        v.set_dailies(vec![
            daily(1, "english", Mode::Words(10)),
            daily(2, "english", Mode::Time(30)),
            daily(3, "english", Mode::Time(15)),
            daily(4, "english_1k", Mode::Words(25)),
        ]);
        v
    }

    #[test]
    fn cycles_modes_languages_and_days() {
        let mut v = view();
        assert_eq!(v.daily().map(|d| d.id), Some(2));
        assert_eq!(
            v.modes_for("english"),
            vec![Mode::Time(15), Mode::Time(30), Mode::Words(10)]
        );
        assert!(v.cycle_mode(1));
        assert_eq!(v.mode, Mode::Words(10));
        assert!(v.cycle_mode(1));
        assert_eq!(v.mode, Mode::Time(15), "wraps");
        assert!(v.cycle_language(1));
        assert_eq!(
            (v.language.as_str(), v.mode),
            ("english_1k", Mode::Words(25))
        );
        assert!(!v.cycle_mode(1), "one mode only");
        assert_eq!(v.shift_day(-1), "2026-09-28");
        assert!(v.dailies_loading && v.dailies.is_empty());
        assert!(v.panes.iter().all(|p| p.loading));
    }

    #[test]
    fn periods_swap_the_boards_and_target() {
        let mut v = view();
        assert_eq!(v.period, Period::Daily);
        assert_eq!(v.target(), Some(Target::Daily(2)));
        v.next_focus();
        assert_eq!(v.focused_board().id, "daily-best");
        v.cycle_period();
        assert_eq!((v.period, v.focus), (Period::AllTime, 0));
        assert_eq!(v.panes.len(), boards::on(Period::AllTime).len());
        assert_eq!(
            v.target(),
            Some(Target::Mode {
                language: "english".into(),
                mode: Mode::Time(30)
            })
        );
        assert!(v.pane_mut("alltime-best").is_some());
        assert!(v.pane_mut("daily-best").is_none(), "not on screen");
        v.cycle_period();
        assert_eq!(v.period, Period::Daily, "wraps");
    }

    #[test]
    fn falls_back_when_the_selection_has_no_daily() {
        let mut v = BoardView::new("2026-09-29".into(), "french".into(), Mode::Words(7));
        v.set_dailies(vec![
            daily(4, "english_1k", Mode::Words(25)),
            daily(1, "english", Mode::Words(10)),
        ]);
        assert_eq!((v.language.as_str(), v.mode), ("english", Mode::Words(10)));
        let mut v = BoardView::new("2026-09-29".into(), "english".into(), Mode::Words(7));
        v.set_dailies(vec![daily(2, "english", Mode::Time(30))]);
        assert_eq!(v.mode, Mode::Time(30));
        v.set_dailies(vec![]);
        assert!(v.daily().is_none());
    }

    #[test]
    fn pages_merge_and_prefetch_near_the_end() {
        let mut p = BoardPane::default();
        p.reset();
        assert!(p.loading);
        let page = |offset: u32, n: u32| Leaderboard {
            board: Board::Best,
            rows: (offset + 1..=offset + n).map(|r| row(r, "u")).collect(),
            offset,
            total: 120,
            me: Some(row(77, "me")),
        };
        p.apply(page(0, 50));
        assert_eq!((p.rows.len(), p.total), (50, 120));
        assert!(!p.wants_more(), "cursor at the top");
        p.selection.select(45);
        assert!(p.wants_more());
        p.apply(page(50, 50));
        assert_eq!(p.rows.len(), 100);
        p.apply(page(50, 50));
        assert_eq!(p.rows.len(), 100, "duplicate page ignored");
        assert_eq!(p.my_index(), Some(76));
        assert_eq!(p.selected().map(|r| r.rank), Some(46));
        p.fail("boom".into());
        assert!(!p.wants_more());
    }
}
