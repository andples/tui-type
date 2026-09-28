//! The leaderboard screen's behaviour: opening it, moving around, switching
//! daily, paging, and the graph view of one run.

use ttyp_core::api::{Board, DailySummary, Leaderboard, ResultDetail};

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
        let (language, mode) = match &self.daily {
            Some(d) => (d.language.clone(), d.mode),
            None => (self.config.language.clone(), self.config.mode),
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

    /// Fetch the first page of both boards for the shown daily.
    fn load_boards(&mut self) {
        let (Some(view), Some(online)) = (&mut self.board, &mut self.online) else {
            return;
        };
        view.reset_panes();
        let Some(daily_id) = view.daily().map(|d| d.id) else {
            for p in &mut view.panes {
                p.loading = view.dailies_loading;
            }
            return;
        };
        for board in [Board::First, Board::Best] {
            online.request(Request::Leaderboard {
                daily_id,
                board,
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
        let Some(daily_id) = view.daily().map(|d| d.id) else {
            return;
        };
        let board = view.focus;
        let pane = view.focused();
        if pane.wants_more() {
            pane.loading = true;
            online.request(Request::Leaderboard {
                daily_id,
                board,
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
                self.screen = Screen::Leaderboard;
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
            B::SwitchBoard => view.toggle_focus(),
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
        daily_id: i64,
        board: Board,
        offset: u32,
        result: Result<Leaderboard, OnlineError>,
    ) {
        let Some(view) = &mut self.board else { return };
        if view.daily().map(|d| d.id) != Some(daily_id) {
            return;
        }
        let pane = view.pane_mut(board);
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
                if self.screen == Screen::Leaderboard {
                    self.screen = Screen::Graph;
                }
            }
            Err(e) => self.notify(format!("graph: {e}")),
        }
    }
}
