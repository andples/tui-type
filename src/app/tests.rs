//! App-level tests over a scratch config and data dir.

use std::time::{Duration, Instant};

use super::{Action, App, Screen, TIMER_TICK};
use crate::config::Paths;

/// Paths in a fresh temp dir holding `config` and `history` copies of
/// one history line.
pub(super) fn scratch(name: &str, config: &str, history: usize) -> Paths {
    let dir = std::env::temp_dir().join(format!("ttyp-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, d) = (dir.join("config"), dir.join("data"));
    std::fs::create_dir_all(&c).unwrap();
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(c.join("config.toml"), config).unwrap();
    let line = r#"{"schema":4,"ts":"2026-09-18T12:19:41Z","mode":{"time":60},"language":"english","punctuation":false,"numbers":false,"wpm":78.3,"raw":96.3,"acc":96.6,"consistency":58.0,"chars":{"correct":177,"incorrect":8,"extra":1,"missed":3},"duration_s":60.0,"missed_chars":{"a":2,"e":1},"typed_chars":{"a":20,"e":30,"t":12}}"#;
    let lines: String = (0..history).map(|_| format!("{line}\n")).collect();
    std::fs::write(d.join("history.jsonl"), lines).unwrap();
    Paths::from_dirs(&c, &d)
}

/// An offline app on the typing screen, with `extra` config lines.
fn app(name: &str, extra: &str) -> App {
    app_with_history(name, extra, 0)
}

fn app_with_history(name: &str, extra: &str, history: usize) -> App {
    let config = format!("server = \"\"\nsplash = false\nupdate_check = false\n{extra}\n");
    let app = App::new(scratch(name, &config, history), None).unwrap();
    assert_eq!(app.screen, Screen::Typing);
    app
}

/// As if a frame had just been drawn.
fn drawn(app: &mut App) {
    app.drawn_clock = app.clock_view(Instant::now());
    app.dirty = false;
}

#[test]
fn a_timed_test_wakes_on_the_second_unless_live_readouts_need_more() {
    let mut a = app("wake", "[mode]\ntime = 30");
    assert_eq!(a.clock_wake(Instant::now()), None, "not running");
    a.dispatch(Action::TypeChar('x'));
    let wake = a.clock_wake(Instant::now()).unwrap();
    assert!(wake > Duration::from_millis(1) && wake <= Duration::from_millis(1001));

    let mut a = app("wake-wpm", "hud = [\"timer\", \"wpm\"]\n[mode]\ntime = 30");
    a.dispatch(Action::TypeChar('x'));
    assert_eq!(a.clock_wake(Instant::now()), Some(TIMER_TICK));

    let mut a = app("wake-words", "[mode]\nwords = 10");
    a.dispatch(Action::TypeChar('x'));
    assert_eq!(
        a.clock_wake(Instant::now()),
        None,
        "words mode has no clock"
    );
}

#[test]
fn ticks_that_change_nothing_on_screen_draw_nothing() {
    let mut a = app("tick", "[mode]\ntime = 30");
    a.dispatch(Action::TypeChar('x'));
    drawn(&mut a);
    a.dispatch(Action::Tick);
    assert!(!a.dirty, "the timer hasn't turned");

    // The drawn timer is out of date: redraw.
    a.drawn_clock.as_mut().unwrap().hud[0] = "31".into();
    a.dispatch(Action::Tick);
    assert!(a.dirty);

    // A notice running out redraws too.
    drawn(&mut a);
    a.notice = Some(("hi".into(), Instant::now() - Duration::from_secs(60)));
    a.dispatch(Action::Tick);
    assert!(a.dirty && a.notice.is_none());

    // Keys always draw.
    drawn(&mut a);
    a.dispatch(Action::TypeChar('y'));
    assert!(a.dirty);
}

#[test]
fn the_pace_target_follows_new_runs() {
    let mut a = app_with_history("pace", "pace = \"pb\"\n[mode]\ntime = 60", 3);
    assert_eq!(a.pace_target(), Some(78.3));
    assert_eq!(a.pace_target(), Some(78.3), "from the memo");
    let mut faster = a.stats.all()[0].clone();
    faster.wpm = 101.0;
    a.stats.append(&faster).unwrap();
    assert_eq!(a.pace_target(), Some(101.0));
    a.config.pace = crate::config::Pace::Wpm(50);
    assert_eq!(a.pace_target(), Some(50.0));
}
