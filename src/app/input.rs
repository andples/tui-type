//! Key event → Action mapping, per screen.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::Screen;
use super::action::Action;
use crate::test::Status;

/// Context needed to interpret a key.
#[derive(Debug, Clone, Copy)]
pub struct InputContext {
    pub screen: Screen,
    pub command_line_open: bool,
    pub test_status: Status,
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
        return Action::ZoomIn;
    }
    if ctrl && matches!(key.code, KeyCode::Char('-') | KeyCode::Char('_')) {
        return Action::ZoomOut;
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
            test_status: status,
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
