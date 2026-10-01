# Handoff: the `surprise` branch (2026-10-01)

The owner asked for "cool things, kept secret, on a branch, quickly, with
subagents". Three agents started in parallel; the owner then set a
four-minute limit, so they were stopped mid-build and their work was
committed as WIP and consolidated here. Nothing is merged to `main` or
released. `cargo check` passes on this branch, but the new code is not
wired in or tested yet.

## The three features

1. **Pace caret**: a ghost caret racing you at a target speed. Setting
   `pace` = `off` (default) / `pb` (personal best for the mode+language) /
   `last` / a number; `:pace` command; profile setting. Pure position logic
   (elapsed, wpm, words → word index, char offset) unit-tested; drawn in
   the theme's `sub` colour, not obscuring text; `pace 87` on the mode line;
   visual only (never touches keylog/scoring). **Status: lost.** The agent's
   worktree was gone when stopped; nothing was committed. Start from scratch.
2. **Stats activity view** (`c9674a8`): a GitHub-style calendar of tests per
   day over the last ~52 weeks (theme-shaded, month labels, today
   highlighted, local dates), a summary line (current/longest streak, total
   tests, total time, best day) and a 30-day wpm sparkline, at the top of
   the stats screen; degrades in small terminals. Pure functions in
   `src/stats/` with tests. **Status: partial**; check what's wired into
   `src/ui/stats.rs`, finish, test with a generated history (`--data-dir`).
3. **Results screen** (`6913ad1`): (a) a ~1.5 s confetti burst in theme
   colours when a test is a new PB (`src/app/celebrate.rs`; seeded RNG, pure
   `frame_at(Instant)`, wakes the event loop only while playing, any key
   ends it; `celebrate = true` setting), (b) a QWERTY keyboard heatmap of
   this test's missed keys (`src/app/misses.rs`; pure words → per-key error
   counts; a `keys` results section, `:results keys off`). **Status:
   partial**: both modules exist but are not declared in `src/app/mod.rs`
   or drawn yet.

## To finish

- Wire each piece in following CLAUDE.md (Screen/Action/dispatch, rendering
  in `src/ui/`, settings in `Config` + `:set`, tick only while animating as
  `app/splash.rs` and `app/idle.rs` do).
- `cargo fmt`, `cargo test --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings`; check each headless with tmux (font size 1,
  temp `--config-dir`/`--data-dir`, `server = ""`).
- Document in README and CLAUDE.md, then show the owner. It's meant as a
  surprise: don't describe it to them before it's done.
- The agents' worktrees (`.claude/worktrees/agent-a0525…`, `agent-a759a…`)
  still hold the same commits; remove them with `git worktree remove` when
  done.
