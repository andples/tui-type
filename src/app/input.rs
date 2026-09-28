//! Key event → Action mapping, per screen.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::Screen;
use super::action::{Action, ProfileAction};
use crate::test::Status;

/// Context needed to interpret a key.
#[derive(Debug, Clone, Copy)]
pub struct InputContext {
    pub screen: Screen,
    pub command_line_open: bool,
    pub slider_open: bool,
    pub test_status: Status,
    pub profile_menu: ProfileInput,
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
        Screen::Results => match key.code {
            KeyCode::Tab | KeyCode::Enter => Action::Restart,
            KeyCode::Esc | KeyCode::Char(':') => Action::OpenCommandLine,
            KeyCode::Char('s') => Action::ShowStats,
            KeyCode::Char('?') => Action::ShowHelp,
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Nop,
        },
        Screen::Profiles => map_profiles(key, ctx.profile_menu),
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
        }
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
