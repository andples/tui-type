//! The leaderboard screen's behaviour: opening it, moving around, switching
//! period, daily, language and mode, paging, and the graph view of one run.

use ttyp_core::api::{DailySummary, Leaderboard, ResultDetail};
use ttyp_core::boards::{Period, Target};

use super::action::BoardAction;
use super::{App, Screen};
use crate::online::board::PAGE;
use crate::online::{BoardView, OnlineError, Request};

impl App {
    /// `:leaderboard`: today's boards for the current language and mode
    /// (or the daily just typed).
    pub(super) fn open_leaderboard(&mut self) {
        let Some(online) = &mut self.online else {
            self.notify("offline: set server in config");
            return;
        };
        let today = chrono::Utc::now().date_naive().to_string();
        // The daily being typed, or the one whose results are on screen,
        // else the config's language and mode.
        let daily_result = self
            .outcome
            .as_ref()
            .filter(|o| o.daily.is_some())
            .map(|o| (o.record.language.clone(), o.record.mode));
        let (language, mode) = match (&self.daily, daily_result) {
            (Some(d), _) => (d.language.clone(), d.mode),
            (None, Some(r)) => r,
            (None, None) => (self.config.language.clone(), self.config.mode),
        };
        let mut view = BoardView::new(today.clone(), language, mode);
        if let Some((date, list)) = &online.dailies
            && *date == today
        {
            view.set_dailies(list.clone());
        } else {
            online.request(Request::DailiesFor(today));
        }
        self.board = Some(view);
        self.load_boards();
        self.push_screen(Screen::Leaderboard);
    }

    /// Fetch the first page of every board on screen.
    fn load_boards(&mut self) {
        let (Some(view), Some(online)) = (&mut self.board, &mut self.online) else {
            return;
        };
        view.reset_panes();
        let Some(target) = view.target() else {
            for p in &mut view.panes {
                p.loading = view.dailies_loading;
            }
            return;
        };
        for board in view.boards() {
            online.request(Request::Leaderboard {
                board: board.id,
                target: target.clone(),
                offset: 0,
                limit: PAGE,
            });
        }
    }

    /// Fetch the next page of the focused board when the cursor nears the end.
    fn fetch_more(&mut self) {
        let (Some(view), Some(online)) = (&mut self.board, &mut self.online) else {
            return;
        };
        let Some(target) = view.target() else {
            return;
        };
        let board = view.focused_board().id;
        let pane = view.focused();
        if pane.wants_more() {
            pane.loading = true;
            online.request(Request::Leaderboard {
                board,
                target,
                offset: pane.rows.len() as u32,
                limit: PAGE,
            });
        }
    }

    pub(super) fn board_action(&mut self, action: BoardAction) {
        use BoardAction as B;
        if action == B::CloseGraph {
            self.graph = None;
            self.graph_loading = false;
            if self.screen == Screen::Graph {
                self.screen = self.graph_from;
            }
            return;
        }
        let Some(view) = &mut self.board else { return };
        match action {
            B::Up => view.focused().selection.move_by(-1),
            B::Down => view.focused().selection.move_by(1),
            B::Top => view.focused().selection.home(),
            B::Bottom => view.focused().selection.end(),
            B::PageUp => view.focused().selection.page(-1),
            B::PageDown => view.focused().selection.page(1),
            B::SwitchBoard => view.next_focus(),
            B::NextPeriod => {
                view.cycle_period();
                self.load_boards();
            }
            B::NextMode | B::PrevMode => {
                if view.cycle_mode(if action == B::NextMode { 1 } else { -1 }) {
                    self.load_boards();
                }
            }
            B::NextLanguage => {
                if view.cycle_language(1) {
                    self.load_boards();
                }
            }
            // All-time boards have no day.
            B::PrevDay | B::NextDay if view.period != Period::Daily => {}
            B::PrevDay | B::NextDay => {
                let date = view.shift_day(if action == B::NextDay { 1 } else { -1 });
                if let Some(o) = &mut self.online {
                    o.request(Request::DailiesFor(date));
                }
            }
            B::Open => {
                let id = view.focused().selected().map(|r| r.result_id);
                if let (Some(id), Some(o)) = (id, &mut self.online) {
                    self.graph_loading = true;
                    o.request(Request::Result(id));
                }
            }
            B::Profile => {
                if let Some(login) = view.focused().selected().map(|r| r.user.clone()) {
                    self.open_user(Some(login));
                }
            }
            B::CloseGraph => {}
        }
        if matches!(action, B::Down | B::Bottom | B::PageDown) {
            self.fetch_more();
        }
    }

    pub(super) fn board_dailies(
        &mut self,
        date: String,
        result: Result<Vec<DailySummary>, OnlineError>,
    ) {
        let today = chrono::Utc::now().date_naive().to_string();
        if let (Ok(list), Some(o)) = (&result, &mut self.online)
            && date == today
        {
            o.dailies = Some((today, list.clone()));
        }
        let Some(view) = &mut self.board else { return };
        if view.date != date {
            return;
        }
        match result {
            Ok(list) => {
                view.set_dailies(list);
                view.error = None;
                self.load_boards();
            }
            Err(e) => {
                view.dailies_loading = false;
                view.error = Some(e.to_string());
                for p in &mut view.panes {
                    p.loading = false;
                }
            }
        }
    }

    pub(super) fn board_page(
        &mut self,
        board: &str,
        target: &Target,
        offset: u32,
        result: Result<Leaderboard, OnlineError>,
    ) {
        let Some(view) = &mut self.board else { return };
        if view.target().as_ref() != Some(target) {
            return;
        }
        let Some(pane) = view.pane_mut(board) else {
            return;
        };
        match result {
            Ok(lb) => pane.apply(lb),
            Err(e) if offset == 0 => pane.fail(e.to_string()),
            Err(_) => pane.loading = false,
        }
    }

    pub(super) fn board_result(&mut self, result: Result<ResultDetail, OnlineError>) {
        if !self.graph_loading {
            return;
        }
        self.graph_loading = false;
        match result {
            Ok(d) => {
                self.graph = Some(d);
                if matches!(self.screen, Screen::Leaderboard | Screen::User) {
                    self.graph_from = self.screen;
                    self.screen = Screen::Graph;
                }
            }
            Err(e) => self.notify(format!("graph: {e}")),
        }
    }
}
