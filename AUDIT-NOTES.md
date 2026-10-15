# Responsiveness audit notes (2026-10-16)

Branch `worktree-agent-adfd8f91c164eda13`, on top of `a02f929` (v2.6.2). Not pushed or merged.

## How it was measured

- **pty driver** (a scratch Python script, not committed): runs `target/release/ttyp` under a pty
  with pixel sizes (140x40 cells, 10x22 px, `TERM=xterm-ghostty`, no `TMUX`), so kitty graphics
  are on. Configs use `server = ""` (or a local socket that never answers), `update_check = false`,
  `font = ""` (system font: JetBrainsMono Nerd Font, 2.5 MB, 12.5k glyphs) and `font_size = 5`.
  It times startup to the first frame, per-key latency, bursts, resizes, and output while idle.
- **Startup phases**: a temporary `TTYP_TIMING=<file>` log of marks in `App::new` and after each
  draw (not committed; re-add it to break startup down again).
- **Frame cost**: `cargo test --release --lib frame_cost -- --ignored --nocapture`
  (`src/app/bench.rs`): an `App` over a 5000-run history, typing into a 120 s test, TestBackend
  140x40, with `Gfx::present` into a buffer for the image case.
- History for the 5k cases: 5000 synthetic runs with `missed_chars`/`typed_chars`.

## Results, per commit

| Commit | Finding | Before | After |
|---|---|---|---|
| 6f915dc | fontdue outlined all 12.5k Nerd Font glyphs on one core (`parallel` feature) | gfx setup 88 ms, first frame 99 ms | 27 ms, 37 ms |
| bbbe327 | `fc-list` spawned on every start for `:font` completions; now `Completions::fonts()` on first use | first frame 37 ms | 28 ms |
| 48df73e | startup font loaded synchronously; now on a thread (`FaceSlot`), waited for only by a frame that draws real-font text | first frame, splash on: 28 ms; splash off: 101 ms (v2.6.2) | 1.4 ms; 30 ms |
| a6cb0cb | a frame per input event; now drains waiting events first (max 256) | 300 keys at once: 300 frames, 208 KB, 39.5 ms | 2 frames, 34 KB, 1.5 ms |
| 7f7f15f | running timed test woke and redrew every 100 ms; now wakes on the whole second (timer only) and skips ticks that leave `ClockView` as drawn | idle in a running test: ~10 frames/s, 2.2 KB/5 s; no-op tick ~100 us | 1 frame/s, 0.4 KB/5 s; 0.4 us |
| 7d2a1fc | image layout measured every char through fontdue each frame; ASCII advance table per px (`Face::advancer`) | render, size 5 images: 58 us | 22 us |
| 3f30b8a | `pace_target` scanned the whole history every frame/tick/wake; memoized on its inputs | tick with pace on: 6.0 us; size 1 render 49 us | 0.5 us; 42 us |
| 99987ba | resize re-rasterized and re-encoded every glyph; encoded uploads cached (8 MB cap). Bytes unchanged: the terminal clear frees images | resize CPU 1.63 ms; SIGWINCH to last byte 2.7 ms | 0.11 ms; 1.0 ms |
| d9b8ce1 | `Summary` of the whole history built at startup but only shown (and rebuilt) on the stats screen | first frame, 5k runs: 16.5 ms | 13.4 ms |
| 59aa1c9 | every 50 ms network-poll tick redrew whatever screen was up; `ClockView::Still` for screens that don't animate | leaderboard waiting 3 s on a stalled server: ~20 frames/s, 2.4 KB | 0 frames, 0 bytes |
| 3c9e650 | screensaver ghost woke and redrew at 30 fps; now wakes per ghost keystroke and skips identical frames | 8 s on the landing screen: 371 frames | 75 frames |

Unchanged throughout: per-key latency end to end (~0.2–0.3 ms median), scoring, keylog,
history and wire format. With timer-only HUD a timed test now ends within ~1 ms of its limit
rather than up to 100 ms late; the engine already clamps the end to the limit.

## Leads not done yet

- **History load**: ~2.4 us per run (12 ms for 5000 runs with char maps), all of startup's
  remaining time at that size. Options: parse on a thread (pace/PB need it at first test start or
  finish), or a cheaper line parser.
- **ratatui frame overhead**: ~60 us of the ~100 us bench frame is `Terminal::draw` (buffer
  reset + diff of 5600 cells on TestBackend); `ui::render` itself is ~20 us of chrome (full-screen
  `Block` fill, palette). Not worth changing at these numbers.
- **Live wpm HUD**: `Metrics::compute` per frame costs ~1–2 us (the per-second vectors); a cheap
  live-wpm path isn't worth it at that size.
- **Typing render**: `layout_lines` and the cell vectors are rebuilt each frame (~4 us at size 1
  besides the advance table); `pace_position_at` builds a `Vec` of word lengths each call. Cheap.
- **Resize bytes**: still ~60–70 KB per resize at size 5 because the screen clear frees images
  (Ghostty does). Avoiding ratatui's clear on resize might let images survive in kitty, but needs
  checking in real terminals.
- **Splash intro** wakes per letter step (by design); the crumble runs at 30 fps (by design).
- The system Nerd Font still takes ~25 ms (fc-match + parallel parse) on the no-splash path; the
  bundled font loads in ~2 ms. Defaulting `font = ""` to a bundled font would change the look, so
  it was left alone.
