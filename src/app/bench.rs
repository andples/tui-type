//! Frame-cost benchmark for the typing screen, run by hand:
//!
//! ```sh
//! cargo test --release --lib frame_cost -- --ignored --nocapture
//! ```
//!
//! Builds an `App` over a scratch config and a 5000-run history, types
//! into a test and times dispatch + render (+ `Gfx::present` into a buffer
//! with real-font images) per keystroke and per timer tick.

use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::tests::scratch;
use super::{Action, App, Screen};
use crate::gfx::Gfx;
use crate::ui;

const TEXT: &str = "the quick brown fox jumps over the lazy dog ";

struct Timing {
    key: Duration,
    tick: Duration,
}

fn run(name: &str, config: &str, images: bool) -> Timing {
    let paths = scratch(name, config, 5000);
    let mut app = App::new(paths, None).unwrap();
    if images {
        app.gfx = Gfx::for_tests((10, 22));
    }
    assert_eq!(app.screen, Screen::Typing);
    let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
    let mut sink = Vec::new();
    // What `App::draw` does, on a test backend.
    let mut frame = |app: &mut App, sink: &mut Vec<u8>| {
        app.drawn_clock = app.clock_view(Instant::now());
        let mut lines = Vec::new();
        term.draw(|f| lines = ui::render(f, app)).unwrap();
        app.gfx.present(&lines, sink).unwrap();
        sink.clear();
    };
    // Warm up: start the test and get some words behind the caret.
    for c in TEXT.chars().cycle().take(100) {
        app.dispatch(Action::TypeChar(c));
        frame(&mut app, &mut sink);
    }
    let n = 400;
    let t = Instant::now();
    for c in TEXT.chars().cycle().take(n) {
        app.dispatch(Action::TypeChar(c));
        frame(&mut app, &mut sink);
    }
    let key = t.elapsed() / n as u32;
    let t = Instant::now();
    for _ in 0..n {
        app.dispatch(Action::Tick);
        if app.dirty {
            frame(&mut app, &mut sink);
            app.dirty = false;
        }
    }
    let tick = t.elapsed() / n as u32;
    // Split: dispatch alone, render alone into a bare buffer.
    let t = Instant::now();
    for c in TEXT.chars().cycle().take(n) {
        app.dispatch(Action::TypeChar(c));
    }
    let d = t.elapsed() / n as u32;
    let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
    let (mut render, mut present) = (Duration::ZERO, Duration::ZERO);
    let t = Instant::now();
    for _ in 0..n {
        let mut lines = Vec::new();
        term.draw(|f| {
            let t = Instant::now();
            lines = ui::render(f, &app);
            render += t.elapsed();
        })
        .unwrap();
        let t = Instant::now();
        app.gfx.present(&lines, &mut sink).unwrap();
        present += t.elapsed();
        sink.clear();
    }
    let all = t.elapsed() / n as u32;

    println!(
        "   dispatch {:?}  render {:?}  present {:?}  whole frame {:?}",
        d,
        render / n as u32,
        present / n as u32,
        all
    );
    Timing { key, tick }
}

#[test]
#[ignore = "benchmark; run by hand with --release --nocapture"]
fn frame_cost() {
    let base = "server = \"\"\nsplash = false\nupdate_check = false\n[mode]\ntime = 120\n";
    let cases = [
        ("size 1", "font_size = 1\ngraphics = \"off\"\n", false),
        (
            "size 5 blocks",
            "font_size = 5\ngraphics = \"off\"\n",
            false,
        ),
        ("size 5 images", "font_size = 5\ngraphics = \"off\"\n", true),
        (
            "size 1 + wpm hud + pace pb",
            "font_size = 1\ngraphics = \"off\"\nhud = [\"timer\", \"wpm\"]\npace = \"pb\"\n",
            false,
        ),
        (
            "size 5 images + wpm hud + pace pb",
            "font_size = 5\ngraphics = \"off\"\nhud = [\"timer\", \"wpm\"]\npace = \"pb\"\n",
            true,
        ),
    ];
    for (name, extra, images) in cases {
        let t = run(
            &name.replace(' ', "_").replace('+', ""),
            &format!("{extra}{base}"),
            images,
        );
        println!(
            "{name:<36} key+frame {:>7.1} µs   tick {:>7.1} µs",
            t.key.as_secs_f64() * 1e6,
            t.tick.as_secs_f64() * 1e6
        );
    }
}
