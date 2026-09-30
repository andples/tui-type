//! Key event → Action mapping, per screen.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::Screen;
use super::action::{Action, BoardAction, CatalogAction, ProfileAction, UserAction};
use super::splash::SplashMenu;
use crate::test::Status;

/// Context needed to interpret a key.
#[derive(Debug, Clone, Copy)]
pub struct InputContext {
    pub screen: Screen,
    pub command_line_open: bool,
    pub slider_open: bool,
    pub test_status: Status,
    pub profile_menu: ProfileInput,
    /// The install screen is asking whether to remove something.
    pub catalog_confirm: bool,
    /// The landing screen's intro is still being typed out.
    pub splash_playing: bool,
    /// What the landing screen offers.
    pub splash_menu: SplashMenu,
}

/// Which part of the profile screen has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileInput {
    List,
    ConfirmDelete,
    EditName,
    EditSettings,
}

pub fn map_key(key: KeyEvent, ctx: InputContext) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::Nop;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    // Global
    if ctrl && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    if ctrl && key.code == KeyCode::Char('l') {
        return Action::Redraw;
    }
    if ctrl && matches!(key.code, KeyCode::Char('=') | KeyCode::Char('+')) {
        return Action::FontBigger;
    }
    if ctrl && matches!(key.code, KeyCode::Char('-') | KeyCode::Char('_')) {
        return Action::FontSmaller;
    }

    if ctx.slider_open {
        return map_slider(key);
    }
    if ctx.command_line_open {
        return map_command_line(key, ctrl, alt);
    }

    match ctx.screen {
        Screen::Typing => map_typing(key, ctrl, alt, ctx.test_status),
        Screen::Splash => map_splash(key, ctrl || alt, ctx.splash_playing, ctx.splash_menu),
        Screen::Results => match key.code {
            KeyCode::Tab | KeyCode::Enter => Action::Restart,
            KeyCode::Esc | KeyCode::Char(':') => Action::OpenCommandLine,
            KeyCode::Char('s') => Action::ShowStats,
            KeyCode::Char('?') => Action::ShowHelp,
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Nop,
        },
        Screen::Profiles => map_profiles(key, ctx.profile_menu),
        Screen::Catalog => map_catalog(key, ctx.catalog_confirm),
        Screen::Login => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::CancelLogin,
            KeyCode::Char(':') => Action::OpenCommandLine,
            _ => Action::Nop,
        },
        Screen::Leaderboard => map_leaderboard(key, ctrl),
        Screen::User => {
            use UserAction as U;
            let u = |a| Action::User(a);
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => u(U::Up),
                KeyCode::Down | KeyCode::Char('j') => u(U::Down),
                KeyCode::Home | KeyCode::Char('g') => u(U::Top),
                KeyCode::End | KeyCode::Char('G') => u(U::Bottom),
                KeyCode::Enter => u(U::Open),
                KeyCode::Esc | KeyCode::Char('q') => u(U::Close),
                KeyCode::Char(':') => Action::OpenCommandLine,
                KeyCode::Char('?') => Action::ShowHelp,
                _ => Action::Nop,
            }
        }
        Screen::Graph => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                Action::Board(BoardAction::CloseGraph)
            }
            KeyCode::Char(':') => Action::OpenCommandLine,
            _ => Action::Nop,
        },
        Screen::Stats | Screen::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Tab | KeyCode::Enter => Action::Back,
            KeyCode::Char(':') => Action::OpenCommandLine,
            KeyCode::Down | KeyCode::Char('j') => Action::ScrollDown,
            KeyCode::Up | KeyCode::Char('k') => Action::ScrollUp,
            KeyCode::Char('?') => Action::ShowHelp,
            _ => Action::Nop,
        },
    }
}

fn map_typing(key: KeyEvent, ctrl: bool, alt: bool, status: Status) -> Action {
    match key.code {
        KeyCode::Esc => Action::OpenCommandLine,
        KeyCode::Tab => Action::Restart,
        // Backspace with ctrl/alt, ctrl+w and ctrl+h all delete a word —
        // terminals disagree on which one they send.
        KeyCode::Backspace if ctrl || alt => Action::DeleteWord,
        KeyCode::Char('w') | KeyCode::Char('h') if ctrl => Action::DeleteWord,
        KeyCode::Backspace => Action::Backspace,
        // `:` opens the command line only when it can't be test input.
        KeyCode::Char(':') if status != Status::Running => Action::OpenCommandLine,
        KeyCode::Char('?') if status != Status::Running => Action::ShowHelp,
        KeyCode::Char(c) if !ctrl && !alt => Action::TypeChar(c),
        _ => Action::Nop,
    }
}

/// The landing screen. While the intro plays the keys aren't on screen
/// yet, so a character is the start of a test; once they show, their
/// letters are taken and every other character still starts the test.
/// Other keys finish the intro first.
fn map_splash(key: KeyEvent, modified: bool, playing: bool, menu: SplashMenu) -> Action {
    if modified {
        return Action::Nop;
    }
    match key.code {
        KeyCode::Tab => return Action::CloseSplash,
        KeyCode::Char(':') => return Action::OpenCommandLine,
        KeyCode::Char('?') => return Action::ShowHelp,
        // Any letter just takes you to the words; it isn't typed.
        KeyCode::Char(c) if playing && c != ' ' => return Action::CloseSplash,
        _ if playing => return Action::SkipIntro,
        _ => {}
    }
    match (key.code, menu) {
        (KeyCode::Up, _) => Action::SplashMove(-1),
        (KeyCode::Down, _) => Action::SplashMove(1),
        // Enter runs whichever row is highlighted.
        (KeyCode::Enter, _) => Action::SplashChoose,
        (KeyCode::Char('l'), SplashMenu::LoggedOut) => Action::Login,
        (KeyCode::Char('d'), SplashMenu::LoggedIn) => Action::Daily,
        (KeyCode::Char('b'), SplashMenu::LoggedIn) => Action::ShowLeaderboard,
        (KeyCode::Esc | KeyCode::Char(' '), _) => Action::CloseSplash,
        (KeyCode::Char(_), _) => Action::CloseSplash,
        _ => Action::Nop,
    }
}

fn map_leaderboard(key: KeyEvent, ctrl: bool) -> Action {
    use BoardAction as B;
    let b = |a| Action::Board(a);
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => b(B::Up),
        KeyCode::Down | KeyCode::Char('j') => b(B::Down),
        KeyCode::Char('g') | KeyCode::Home => b(B::Top),
        KeyCode::Char('G') | KeyCode::End => b(B::Bottom),
        KeyCode::PageUp => b(B::PageUp),
        KeyCode::PageDown => b(B::PageDown),
        KeyCode::Char('u') if ctrl => b(B::PageUp),
        KeyCode::Char('d') if ctrl => b(B::PageDown),
        KeyCode::Tab | KeyCode::BackTab => b(B::SwitchBoard),
        KeyCode::Left => b(B::PrevMode),
        KeyCode::Right => b(B::NextMode),
        KeyCode::Char('l') => b(B::NextLanguage),
        KeyCode::Char('[') => b(B::PrevDay),
        KeyCode::Char(']') => b(B::NextDay),
        KeyCode::Enter => b(B::Open),
        KeyCode::Char('p') => b(B::Profile),
        KeyCode::Esc | KeyCode::Char('q') => Action::Back,
        KeyCode::Char(':') => Action::OpenCommandLine,
        KeyCode::Char('?') => Action::ShowHelp,
        _ => Action::Nop,
    }
}

fn map_catalog(key: KeyEvent, confirm: bool) -> Action {
    use CatalogAction as C;
    let c = |a| Action::Catalog(a);
    if confirm {
        return match key.code {
            KeyCode::Char('y') => c(C::ConfirmRemove),
            _ => c(C::CancelRemove),
        };
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => c(C::Up),
        KeyCode::Down | KeyCode::Char('j') => c(C::Down),
        KeyCode::Home | KeyCode::Char('g') => c(C::Top),
        KeyCode::End | KeyCode::Char('G') => c(C::Bottom),
        KeyCode::Tab
        | KeyCode::BackTab
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Char('h')
        | KeyCode::Char('l') => c(C::SwitchTab),
        KeyCode::Enter | KeyCode::Char(' ') => c(C::Use),
        KeyCode::Char('i') => c(C::Install),
        KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete => c(C::Remove),
        KeyCode::Char('r') => c(C::Refresh),
        KeyCode::Esc | KeyCode::Char('q') => Action::Back,
        KeyCode::Char(':') => Action::OpenCommandLine,
        KeyCode::Char('?') => Action::ShowHelp,
        _ => Action::Nop,
    }
}

fn map_profiles(key: KeyEvent, mode: ProfileInput) -> Action {
    use ProfileAction as P;
    let p = |a| Action::Profile(a);
    // Arrows move everywhere; the editor's name row takes letters as text.
    match key.code {
        KeyCode::Up | KeyCode::BackTab => return p(P::Up),
        KeyCode::Down | KeyCode::Tab => return p(P::Down),
        _ => {}
    }
    match mode {
        ProfileInput::List => match key.code {
            KeyCode::Char('k') => p(P::Up),
            KeyCode::Char('j') => p(P::Down),
            KeyCode::Enter | KeyCode::Char(' ') => p(P::Toggle),
            KeyCode::Char('n') | KeyCode::Char('a') => p(P::New),
            KeyCode::Char('e') => p(P::Edit),
            KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete => p(P::Delete),
            KeyCode::Esc | KeyCode::Char('q') => Action::Back,
            KeyCode::Char(':') => Action::OpenCommandLine,
            KeyCode::Char('?') => Action::ShowHelp,
            _ => Action::Nop,
        },
        ProfileInput::ConfirmDelete => match key.code {
            KeyCode::Char('y') => p(P::ConfirmDelete),
            _ => p(P::CancelDelete),
        },
        ProfileInput::EditName => match key.code {
            KeyCode::Enter => p(P::Save),
            KeyCode::Esc => p(P::Cancel),
            KeyCode::Backspace => p(P::Backspace),
            KeyCode::Char(c) => p(P::Insert(c)),
            _ => Action::Nop,
        },
        ProfileInput::EditSettings => match key.code {
            KeyCode::Char('k') => p(P::Up),
            KeyCode::Char('j') => p(P::Down),
            KeyCode::Char(' ') | KeyCode::Char('x') => p(P::Check),
            KeyCode::Left | KeyCode::Char('h') => p(P::Cycle(-1)),
            KeyCode::Right | KeyCode::Char('l') => p(P::Cycle(1)),
            KeyCode::Char('u') => p(P::Capture),
            KeyCode::Char('A') => p(P::CaptureAll),
            KeyCode::Enter => p(P::Save),
            KeyCode::Esc | KeyCode::Char('q') => p(P::Cancel),
            _ => Action::Nop,
        },
    }
}

fn map_slider(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Left
        | KeyCode::Down
        | KeyCode::Char('h')
        | KeyCode::Char('j')
        | KeyCode::Char('-') => Action::SliderDec,
        KeyCode::Right
        | KeyCode::Up
        | KeyCode::Char('l')
        | KeyCode::Char('k')
        | KeyCode::Char('+')
        | KeyCode::Char('=') => Action::SliderInc,
        KeyCode::Home | KeyCode::Char('0') => Action::SliderMin,
        KeyCode::End | KeyCode::Char('$') => Action::SliderMax,
        KeyCode::Enter | KeyCode::Char(' ') => Action::SliderConfirm,
        KeyCode::Esc | KeyCode::Char('q') => Action::SliderCancel,
        _ => Action::Nop,
    }
}

fn map_command_line(key: KeyEvent, ctrl: bool, alt: bool) -> Action {
    match key.code {
        KeyCode::Esc => Action::CloseCommandLine,
        KeyCode::Enter => Action::CmdSubmit,
        KeyCode::Tab => Action::CmdAccept,
        KeyCode::BackTab => Action::CmdSelectPrev,
        KeyCode::Down => Action::CmdSelectNext,
        KeyCode::Up => Action::CmdSelectPrev,
        KeyCode::Char('n') | KeyCode::Char('j') if ctrl => Action::CmdSelectNext,
        KeyCode::Char('p') | KeyCode::Char('k') if ctrl => Action::CmdSelectPrev,
        KeyCode::Backspace if ctrl || alt => Action::CmdDeleteWord,
        KeyCode::Char('w') | KeyCode::Char('h') if ctrl => Action::CmdDeleteWord,
        KeyCode::Char('u') if ctrl => Action::CmdDeleteWord,
        KeyCode::Backspace => Action::CmdBackspace,
        KeyCode::Left => Action::CmdLeft,
        KeyCode::Right => Action::CmdRight,
        KeyCode::Home => Action::CmdHome,
        KeyCode::End => Action::CmdEnd,
        KeyCode::Char('a') if ctrl => Action::CmdHome,
        KeyCode::Char('e') if ctrl => Action::CmdEnd,
        KeyCode::Char(c) if !ctrl && !alt => Action::CmdInsert(c),
        _ => Action::Nop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn ctx(screen: Screen, open: bool, status: Status) -> InputContext {
        InputContext {
            screen,
            command_line_open: open,
            slider_open: false,
            test_status: status,
            profile_menu: ProfileInput::List,
            catalog_confirm: false,
            splash_playing: false,
            splash_menu: SplashMenu::Offline,
        }
    }

    fn splash(playing: bool, menu: SplashMenu, code: KeyCode) -> Action {
        let mut c = ctx(Screen::Splash, false, Status::Idle);
        c.splash_playing = playing;
        c.splash_menu = menu;
        map_key(key(code, KeyModifiers::NONE), c)
    }

    #[test]
    fn splash_intro_letters_go_to_the_words() {
        use SplashMenu as M;
        // The menu isn't showing yet: any letter, menu letters included,
        // just moves to the typing screen without being typed.
        for (menu, c) in [(M::LoggedOut, 'l'), (M::LoggedIn, 'd'), (M::Offline, 'x')] {
            assert_eq!(splash(true, menu, KeyCode::Char(c)), Action::CloseSplash);
        }
        for code in [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char(' '),
            KeyCode::Up,
        ] {
            assert_eq!(splash(true, M::LoggedOut, code), Action::SkipIntro);
        }
        assert_eq!(splash(true, M::Offline, KeyCode::Tab), Action::CloseSplash);
        assert_eq!(
            splash(true, M::Offline, KeyCode::Char(':')),
            Action::OpenCommandLine
        );
    }

    #[test]
    fn splash_menu_keys_follow_login_state() {
        use SplashMenu as M;
        let ch = KeyCode::Char;
        assert_eq!(
            splash(false, M::LoggedOut, KeyCode::Enter),
            Action::SplashChoose
        );
        assert_eq!(
            splash(false, M::LoggedIn, KeyCode::Down),
            Action::SplashMove(1)
        );
        assert_eq!(
            splash(false, M::LoggedIn, KeyCode::Up),
            Action::SplashMove(-1)
        );
        assert_eq!(splash(false, M::LoggedOut, ch('l')), Action::Login);
        // A letter that isn't a menu key moves to the words, untyped.
        assert_eq!(splash(false, M::LoggedOut, ch('d')), Action::CloseSplash);
        assert_eq!(splash(false, M::LoggedIn, ch('d')), Action::Daily);
        assert_eq!(splash(false, M::LoggedIn, ch('b')), Action::ShowLeaderboard);
        assert_eq!(splash(false, M::LoggedIn, ch('l')), Action::CloseSplash);
        assert_eq!(
            splash(false, M::Offline, KeyCode::Enter),
            Action::SplashChoose
        );
        assert_eq!(splash(false, M::Offline, ch('l')), Action::CloseSplash);
        assert_eq!(splash(false, M::Offline, ch('?')), Action::ShowHelp);
        assert_eq!(splash(false, M::Offline, KeyCode::Esc), Action::CloseSplash);
        assert_eq!(splash(false, M::Offline, KeyCode::Backspace), Action::Nop);
        let mut c = ctx(Screen::Splash, false, Status::Idle);
        c.splash_menu = M::LoggedIn;
        assert_eq!(
            map_key(key(KeyCode::Char('d'), KeyModifiers::CONTROL), c),
            Action::Nop
        );
        assert_eq!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL), c),
            Action::Quit
        );
    }

    #[test]
    fn colon_is_input_while_running() {
        let c = ctx(Screen::Typing, false, Status::Running);
        assert_eq!(
            map_key(key(KeyCode::Char(':'), KeyModifiers::NONE), c),
            Action::TypeChar(':')
        );
        let c = ctx(Screen::Typing, false, Status::Idle);
        assert_eq!(
            map_key(key(KeyCode::Char(':'), KeyModifiers::NONE), c),
            Action::OpenCommandLine
        );
    }

    #[test]
    fn word_delete_variants() {
        let c = ctx(Screen::Typing, false, Status::Running);
        for k in [
            key(KeyCode::Backspace, KeyModifiers::CONTROL),
            key(KeyCode::Backspace, KeyModifiers::ALT),
            key(KeyCode::Char('w'), KeyModifiers::CONTROL),
        ] {
            assert_eq!(map_key(k, c), Action::DeleteWord);
        }
    }

    #[test]
    fn profile_name_row_takes_letters() {
        let mut c = ctx(Screen::Profiles, false, Status::Idle);
        let j = key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(map_key(j, c), Action::Profile(ProfileAction::Down));
        c.profile_menu = ProfileInput::EditName;
        assert_eq!(map_key(j, c), Action::Profile(ProfileAction::Insert('j')));
        c.profile_menu = ProfileInput::ConfirmDelete;
        assert_eq!(map_key(j, c), Action::Profile(ProfileAction::CancelDelete));
    }

    #[test]
    fn command_line_captures_keys() {
        let c = ctx(Screen::Typing, true, Status::Running);
        assert_eq!(
            map_key(key(KeyCode::Char('x'), KeyModifiers::NONE), c),
            Action::CmdInsert('x')
        );
        assert_eq!(
            map_key(key(KeyCode::Enter, KeyModifiers::NONE), c),
            Action::CmdSubmit
        );
        assert_eq!(
            map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL), c),
            Action::Quit
        );
    }
}
