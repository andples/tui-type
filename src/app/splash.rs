//! The landing screen shown on start (`splash = true`): the logo typed out
//! letter by letter, then a tagline and a few keys suited to the online
//! state. Timing is pure (`Splash::frame_at` takes an `Instant`); the event
//! loop wakes for the next frame via `Splash::next_change_at` and stops
//! once the intro is done.

use std::time::{Duration, Instant};

use super::action::Action;
use super::{App, Screen};
use crate::test::Status;

/// The word the intro types out.
pub const LOGO: &str = "ttyp";
/// A beat with only the caret showing before the first letter.
const FIRST_LETTER: Duration = Duration::from_millis(150);
/// Time between letters.
const LETTER_STEP: Duration = Duration::from_millis(110);
/// Pause after the last letter before the tagline and keys appear.
const MENU_DELAY: Duration = Duration::from_millis(220);

/// What the landing screen offers, from the online state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashMenu {
    /// `server = ""`: no online play.
    Offline,
    LoggedOut,
    LoggedIn,
}

/// One row of the landing menu: its shortcut key, label and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashItem {
    pub key: &'static str,
    pub label: &'static str,
    pub action: SplashChoice,
}

/// What a landing-menu row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashChoice {
    StartTyping,
    Help,
    Login,
    /// Open the command line on `daily ` so the modes are listed.
    Daily,
    Leaderboard,
}

impl SplashChoice {
    pub fn action(self) -> Action {
        match self {
            SplashChoice::StartTyping => Action::CloseSplash,
            SplashChoice::Help => Action::ShowHelp,
            SplashChoice::Login => Action::Login,
            SplashChoice::Daily => Action::Daily,
            SplashChoice::Leaderboard => Action::ShowLeaderboard,
        }
    }
}

const fn item(key: &'static str, label: &'static str, action: SplashChoice) -> SplashItem {
    SplashItem { key, label, action }
}

const OFFLINE: [SplashItem; 2] = [
    item("tab", "start typing", SplashChoice::StartTyping),
    item("?", "help", SplashChoice::Help),
];
const LOGGED_OUT: [SplashItem; 2] = [
    item("l", "login with github", SplashChoice::Login),
    item("tab", "start typing", SplashChoice::StartTyping),
];
const LOGGED_IN: [SplashItem; 3] = [
    item("d", "today's daily", SplashChoice::Daily),
    item("b", "leaderboard", SplashChoice::Leaderboard),
    item("tab", "start typing", SplashChoice::StartTyping),
];

impl SplashMenu {
    /// The rows, the primary one first (highlighted at the start).
    pub fn items(self) -> &'static [SplashItem] {
        match self {
            SplashMenu::Offline => &OFFLINE,
            SplashMenu::LoggedOut => &LOGGED_OUT,
            SplashMenu::LoggedIn => &LOGGED_IN,
        }
    }
}

/// One moment of the intro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashFrame {
    /// Letters of `LOGO` shown so far.
    pub letters: usize,
    /// Tagline and keys are showing; the intro is over.
    pub done: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Splash {
    started: Instant,
    skipped: bool,
    /// The highlighted menu row; enter runs it.
    pub selected: usize,
}

impl Splash {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            skipped: false,
            selected: 0,
        }
    }

    /// Move the highlight over `len` rows, wrapping.
    pub fn move_by(&mut self, delta: isize, len: usize) {
        if len > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
        }
    }

    /// Jump to the end of the intro.
    pub fn skip(&mut self) {
        self.skipped = true;
    }

    fn letter_at(i: usize) -> Duration {
        FIRST_LETTER + LETTER_STEP * i as u32
    }

    fn menu_at() -> Duration {
        Self::letter_at(LOGO.len() - 1) + MENU_DELAY
    }

    pub fn frame_at(&self, now: Instant) -> SplashFrame {
        let t = now.saturating_duration_since(self.started);
        if self.skipped || t >= Self::menu_at() {
            return SplashFrame {
                letters: LOGO.len(),
                done: true,
            };
        }
        let letters = (0..LOGO.len())
            .take_while(|&i| t >= Self::letter_at(i))
            .count();
        SplashFrame {
            letters,
            done: false,
        }
    }

    /// How long until the picture changes; `None` once the intro is over,
    /// so the event loop goes back to sleeping until a key.
    pub fn next_change_at(&self, now: Instant) -> Option<Duration> {
        if self.skipped {
            return None;
        }
        let t = now.saturating_duration_since(self.started);
        (0..LOGO.len())
            .map(Self::letter_at)
            .chain(std::iter::once(Self::menu_at()))
            .find(|&at| at > t)
            .map(|at| at - t)
    }
}

/// Actions that leave the landing screen for the typing screen before
/// they run, so the first key lands where it was meant to.
pub(super) fn leaves_splash(action: &Action) -> bool {
    matches!(
        action,
        Action::CloseSplash
            | Action::TypeChar(_)
            | Action::Restart
            | Action::OpenCommandLine
            | Action::ShowHelp
            | Action::ShowStats
            | Action::Login
            | Action::Daily
            | Action::ShowLeaderboard
    )
}

impl App {
    /// The landing screen's offer for the current online state.
    pub fn splash_menu(&self) -> SplashMenu {
        match &self.online {
            None => SplashMenu::Offline,
            Some(o) if o.logged_in() => SplashMenu::LoggedIn,
            Some(_) => SplashMenu::LoggedOut,
        }
    }

    /// The intro's current frame; finished when no intro is playing.
    pub fn splash_frame(&self) -> SplashFrame {
        match &self.splash {
            Some(s) => s.frame_at(Instant::now()),
            None => SplashFrame {
                letters: LOGO.len(),
                done: true,
            },
        }
    }

    /// Enter on the landing menu: the highlighted row's action.
    pub(super) fn splash_choice(&self) -> Action {
        let items = self.splash_menu().items();
        let i = self
            .splash
            .map_or(0, |s| s.selected)
            .min(items.len().saturating_sub(1));
        items
            .get(i)
            .map_or(Action::CloseSplash, |it| it.action.action())
    }

    /// Back to the landing screen after `HOME_AFTER` without a key: the
    /// intro is skipped and a finished test is replaced by a fresh one, so
    /// the first key starts typing.
    pub(super) fn go_home(&mut self, now: Instant) {
        if self.cmd_open {
            self.close_command_line();
        }
        if self.engine.is_finished() {
            self.restart();
        }
        let mut s = Splash::new(now);
        s.skip();
        self.splash = Some(s);
        self.screen = Screen::Splash;
        self.idle.touch(now);
    }

    /// Whether the quiet time sends this screen home: the landing screen
    /// is on, and no test, sign-in, edit or slider is under way. An open
    /// command line is closed.
    pub(super) fn home_allowed(&self) -> bool {
        self.config.splash
            && self.slider.is_none()
            && self.engine.status() != Status::Running
            && !matches!(
                self.screen,
                Screen::Splash
                    | Screen::Login
                    | Screen::Profiles
                    | Screen::Catalog
                    | Screen::Modules
            )
    }

    pub(super) fn close_splash(&mut self) {
        self.splash = None;
        if self.screen == Screen::Splash {
            self.screen = Screen::Typing;
            self.previous_screen = Screen::Typing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn letters_appear_one_by_one_then_the_menu() {
        let t0 = Instant::now();
        let s = Splash::new(t0);
        let f = |n| s.frame_at(t0 + ms(n));
        assert_eq!(f(0).letters, 0);
        assert_eq!(f(149).letters, 0);
        assert_eq!(f(150).letters, 1);
        assert_eq!(f(260).letters, 2);
        assert_eq!(f(479).letters, 3);
        assert_eq!(f(480).letters, 4);
        assert!(!f(699).done);
        assert!(f(700).done);
        assert_eq!(f(5000).letters, 4);
        // A clock before the start (it can't happen, but it mustn't panic).
        if let Some(before) = t0.checked_sub(ms(5)) {
            assert_eq!(s.frame_at(before).letters, 0);
        }
    }

    #[test]
    fn whole_intro_is_short() {
        assert!(Splash::menu_at() <= ms(1000));
    }

    #[test]
    fn next_change_points_at_the_next_frame_and_stops() {
        let t0 = Instant::now();
        let s = Splash::new(t0);
        assert_eq!(s.next_change_at(t0), Some(ms(150)));
        assert_eq!(s.next_change_at(t0 + ms(150)), Some(ms(110)));
        assert_eq!(s.next_change_at(t0 + ms(500)), Some(ms(200)));
        assert_eq!(s.next_change_at(t0 + ms(700)), None);
        // Waking where it said always shows a new frame.
        let mut now = t0;
        let mut frames = vec![s.frame_at(now)];
        while let Some(d) = s.next_change_at(now) {
            now += d;
            let f = s.frame_at(now);
            assert_ne!(Some(&f), frames.last());
            frames.push(f);
        }
        assert_eq!(frames.len(), LOGO.len() + 2);
    }

    #[test]
    fn skip_finishes_at_once() {
        let t0 = Instant::now();
        let mut s = Splash::new(t0);
        s.skip();
        let f = s.frame_at(t0);
        assert!(f.done);
        assert_eq!(f.letters, LOGO.len());
        assert_eq!(s.next_change_at(t0), None);
    }

    #[test]
    fn menus_start_with_their_primary_action() {
        assert_eq!(SplashMenu::LoggedOut.items()[0].action, SplashChoice::Login);
        assert_eq!(SplashMenu::LoggedIn.items()[0].action, SplashChoice::Daily);
        for m in [
            SplashMenu::Offline,
            SplashMenu::LoggedOut,
            SplashMenu::LoggedIn,
        ] {
            assert!(
                m.items()
                    .iter()
                    .any(|it| it.action == SplashChoice::StartTyping)
            );
        }
    }

    #[test]
    fn the_highlight_wraps_and_picks_the_action() {
        let mut s = Splash::new(Instant::now());
        let items = SplashMenu::LoggedIn.items();
        assert_eq!(s.selected, 0);
        s.move_by(-1, items.len());
        assert_eq!(items[s.selected].action, SplashChoice::StartTyping);
        s.move_by(1, items.len());
        s.move_by(1, items.len());
        assert_eq!(items[s.selected].action.action(), Action::ShowLeaderboard);
        assert_eq!(SplashChoice::Daily.action(), Action::Daily);
    }

    #[test]
    fn only_intentful_actions_leave() {
        assert!(leaves_splash(&Action::TypeChar('a')));
        assert!(leaves_splash(&Action::Login));
        assert!(!leaves_splash(&Action::SkipIntro));
        assert!(!leaves_splash(&Action::Tick));
        assert!(!leaves_splash(&Action::FontBigger));
    }
}
