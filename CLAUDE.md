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
- `src/ui/` — rendering only; one file per screen.

## Conventions

- Every state change goes through `App::dispatch(Action)`.
- Engine methods that depend on time take an `Instant` (`*_at`) so tests are deterministic.
- Config changes are saved immediately via `App::save_config`.
- Adding a command: add a `CommandSpec` to `COMMANDS`, a `Command` variant, a `parse` arm, and an `App::execute` arm.
- Adding a built-in theme/language: add the TOML under `assets/` and its `include_str!` to the `BUILTIN` list.
