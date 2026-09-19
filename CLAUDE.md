# ttyp

Monkeytype-style TUI typing test in Rust (ratatui + crossterm). Binary: `ttyp`.

## Commands

```sh
cargo test                                   # unit tests (core is UI-free)
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo run                                    # dev build, real config dirs
cargo run -- --config-dir /tmp/c --data-dir /tmp/d   # sandboxed run
```

Headless manual check: `tmux new -d -s t -x 100 -y 28 "target/release/ttyp --config-dir X --data-dir Y"`,
then `tmux send-keys -t t -l 'text'` and `tmux capture-pane -t t -p`.

## Layout

- `src/test/` — engine (state machine), generator (`WordGenerator` trait), metrics, mode. No UI.
- `src/command/` — command specs + parser (`mod.rs`), palette/fuzzy state (`palette.rs`).
- `src/config/` — `Config` (TOML, `#[serde(default)]`), `Paths` (XDG).
- `src/theme/`, `src/language/` — registries: built-ins via `include_str!` from `assets/`, user files in config dir override by name.
- `src/stats/` — `StatsStore` trait, `LocalJsonlStore`, `Summary`.
- `src/app/` — `App`, `Action` enum, key→action mapping, event loop.
- `src/ui/` — rendering only; one file per screen. `bigtext.rs` rasterizes the 4×6 pixel font (`font.rs`) into block characters for `fontsize` 2–5 when graphics are off; `slider.rs` is the bottom-line numeric picker.
- `src/gfx/` — real-font text for `fontsize` 2–5 via the kitty graphics protocol. `raster.rs` draws one RGBA image per distinct glyph (char + colours) with fontdue; `kitty.rs` encodes upload/place/delete escapes; `fonts.rs` resolves the `font` setting (path, config `fonts/` dir, bundled font, fontconfig family). `typing::render` returns `ImageLine`s; `App::draw` hands them to `Gfx::present` after ratatui draws, inside a synchronized update. `present` diffs placements by pixel position, so a keystroke only moves a few placements; images are purged when size/font/cell size change.

Images can't be seen in tmux. To check graphics output, run ttyp under a pty that reports pixel sizes (`TIOCSWINSZ` with xpixel/ypixel, `TERM=xterm-ghostty`), then replay the `ESC _G` commands: `a=t` uploads (base64 → zlib → RGBA), `a=p` placements at the preceding cursor position plus `X`/`Y` pixels, `a=d` deletes.

## Conventions

- Every state change goes through `App::dispatch(Action)`.
- Engine methods that depend on time take an `Instant` (`*_at`) so tests are deterministic.
- Config changes are saved immediately via `App::save_config`.
- Adding a command: add a `CommandSpec` to `COMMANDS`, a `Command` variant, a `parse` arm, and an `App::execute` arm.
- Numeric settings use `ArgKind::Slider`: `Command::X(None)` opens the slider (`App::open_slider`), `Some(n)` sets directly.
- Adding a built-in theme/language: add the TOML under `assets/` and its `include_str!` to the `BUILTIN` list.
- Adding a bundled font: put `<slug>-latin.ttf`, `<slug>-latin-ext.ttf` and `<slug>.LICENSE` in `assets/fonts/` (Fontsource: `cdn.jsdelivr.net/fontsource/fonts/<slug>@latest/<subset>-400-normal.ttf`) and a `bundled!` line to `BUNDLED`.
