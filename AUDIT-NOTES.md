# Responsiveness audit notes (2026-10-16)

Branch `worktree-agent-adfd8f91c164eda13`, rebased onto local `main` (`8a28c32`, which adds the raw/acc/errors HUD items). Not pushed or merged.

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
| f681882 | fontdue outlined all 12.5k Nerd Font glyphs on one core (`parallel` feature) | gfx setup 88 ms, first frame 99 ms | 27 ms, 37 ms |
| 045b814 | `fc-list` spawned on every start for `:font` completions; now `Completions::fonts()` on first use | first frame 37 ms | 28 ms |
| 042271f | startup font loaded synchronously; now on a thread (`FaceSlot`), waited for only by a frame that draws real-font text | first frame, splash on: 28 ms; splash off: 101 ms (v2.6.2) | 1.4 ms; 30 ms |
| cc4d937 | a frame per input event; now drains waiting events first (max 256) | 300 keys at once: 300 frames, 208 KB, 39.5 ms | 2 frames, 34 KB, 1.5 ms |
| d7a1a7a | running timed test woke and redrew every 100 ms; now wakes on the whole second (timer only) and skips ticks that leave `ClockView` as drawn | idle in a running test: ~10 frames/s, 2.2 KB/5 s; no-op tick ~100 us | 1 frame/s, 0.4 KB/5 s; 0.4 us |
| bd32262 | image layout measured every char through fontdue each frame; ASCII advance table per px (`Face::advancer`) | render, size 5 images: 58 us | 22 us |
| 64556c8 | `pace_target` scanned the whole history every frame/tick/wake; memoized on its inputs | tick with pace on: 6.0 us; size 1 render 49 us | 0.5 us; 42 us |
| f09ee4e | resize re-rasterized and re-encoded every glyph; encoded uploads cached (8 MB cap). Bytes unchanged: the terminal clear frees images | resize CPU 1.63 ms; SIGWINCH to last byte 2.7 ms | 0.11 ms; 1.0 ms |
| 3ea7863 | `Summary` of the whole history built at startup but only shown (and rebuilt) on the stats screen | first frame, 5k runs: 16.5 ms | 13.4 ms |
| 3db6233 | every 50 ms network-poll tick redrew whatever screen was up; `ClockView::Still` for screens that don't animate | leaderboard waiting 3 s on a stalled server: ~20 frames/s, 2.4 KB | 0 frames, 0 bytes |
| 62139e7 | screensaver ghost woke and redrew at 30 fps; now wakes per ghost keystroke and skips identical frames | 8 s on the landing screen: 371 frames | 75 frames |

After the rebase, `App::clock_wake` follows `HudItem::ticks()`: any shown ticking item other than
the timer (live wpm, raw wpm) or `min_wpm` keeps the 100 ms tick; the timer alone wakes once a
second in a timed test; acc/errors only change on keys and add no wakes (tested in
`app::tests`). Idle 3 s in a running 60 s test with the default HUD: 1275 -> 252 bytes.

Unchanged throughout: per-key latency end to end (~0.2–0.3 ms median), scoring, keylog,
history and wire format. With timer-only HUD a timed test now ends within ~1 ms of its limit
rather than up to 100 ms late; the engine already clamps the end to the limit.

## Still open, and why

- **History load** (~2.4 us per run; 12 ms for 5000 runs with char maps; ~0.05 ms for a typical
  20-run history). Moving it to a thread wouldn't help the common case: a logged-in start reads
  it at once (`start_sync` -> `refresh_bests` sends the bests the account lacks), and the idle
  typing screen's mode line asks for the pace target when `pace` is on. Worth it only with very
  large histories; it would need a lazily joined store like `gfx::FaceSlot` and the
  "skipped corrupt lines" notice reported late.
- **Resize bytes**: still ~60–70 KB per resize at size 5. ratatui's autoresize clears the screen
  (ED 2) and, as far as I know, Ghostty frees kitty images on that clear; whether kitty keeps
  them couldn't be checked headlessly (no kitty here, Ghostty needs a display). Skipping the clear
  is left until it can be checked in both terminals.
- **ratatui frame overhead**: ~60 us of the ~100 us bench frame is `Terminal::draw` (buffer reset
  + diff); `ui::render` chrome ~20 us. Not worth changing.
- **Live/raw wpm HUD**: `Metrics::compute` per frame ~1–2 us; not worth a separate path.
- **Typing render**: `layout_lines`, the cell vectors and `pace_position_at`'s word-length `Vec`
  are rebuilt each frame (~4 us). Cheap.
- **Splash intro** wakes per letter step and the crumble at 30 fps, by design.
- **Default font**: the system Nerd Font still costs ~25 ms on the no-splash path (bundled: ~2 ms).
  Changing the default changes the look; owner's call.
