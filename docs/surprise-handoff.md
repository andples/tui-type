# Surprise! Three new things in ttyp

Everything is on the `surprise` branch (worktree
`~/Projects/tui-type-surprise`), one commit per feature, all green:
`cargo fmt`, `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`. Nothing is merged to `main`, pushed or
released, and the server is untouched (no core, wire or history changes).

| | feature | commit |
|---|---|---|
| 1 | Results screen: new-best confetti + missed-keys heatmap | `6447caa` |
| 2 | Stats screen: activity calendar, streaks, 30-day wpm sparkline | `a13dd6f` |
| 3 | Pace caret: a ghost racing you through the words | `6158242` |

Everything is drawn in the active theme's colours (shades are blends of
two theme colours, never fixed colours), works at every font size, gets
out of the way in small terminals, and is purely visual: scoring, the
keylog, daily submissions and `history.jsonl` are exactly as before.

To try it all in a sandbox:

```sh
cd ~/Projects/tui-type-surprise
cargo build --release
mkdir -p /tmp/s/c && printf 'server = ""\nsplash = false\n' > /tmp/s/c/config.toml
target/release/ttyp --config-dir /tmp/s/c --data-dir /tmp/s/d
```

---

## 1. Results screen: confetti and a keyboard heatmap

**Confetti on a new personal best.** Beat your best for the mode and
language and the "new best" text throws a burst of sparks (`* + · • ✦ °`)
in the theme's accent, text and correct colours. They fly out, arc and fall
under gravity, burn down to `sub`-coloured embers and are gone in about a
second and a half. They only land on empty cells away from text, so the
numbers stay readable. Any key ends it early, and keys like `tab` still do
their thing. The event loop only wakes while sparks are in the air.

**Missed keys.** Under the chart there's a QWERTY keyboard with every key
shaded by how often you got its character wrong in this test, plus the
worst offenders by name. The worst keys are a solid `error` block, then
bold `error`, then `error_extra`, and clean keys are `sub`. Shifted
characters count on their key (`!` is `1`). The number row appears only
when it's involved, and characters that aren't on the keyboard (accents,
other scripts) are counted as `other`. It's left out on a clean run and
when the terminal is too short for it.

How to try:

- `:words 10`, type it with a few mistakes. The first run in a fresh
  `--data-dir` is always a new best, so you get the confetti.
- `:results keys off` / `on` hides or shows the keyboard (also
  `[results] keys` in the config, or a profile).
- `:set celebrate off` turns the confetti off (`celebrate = false`, or a
  profile).

Capture (100×30, 0.3 s after finishing a 10-word test, default theme; `e`,
`i` and `n` are the solid red keys, `t` the dim one):

```
          ttyp
                     *✦     °• ·••✦
          wpm        acc  *·• •✦ °
          539        74%  new best   •·
                                ✦
          raw 1682  ·  con 100%  ·  chars 36/7/0/0  ·  words 10  ·  english

          1240│                                                                          ⠈
              │
              │
              │                                                                          ⢀
          620 │
              │
              │
          0   │
              └───────────────────────────────────────────────────────────────────────────
             0s                                                                         1s

          missed keys   e 2  ·  i 2  ·  n 2  ·  t 1
             q   w   e   r   t   y   u   i   o   p   [   ]
              a   s   d   f   g   h   j   k   l   ;   '
                z   x   c   v   b   n   m   ,   .   /

          tab  next   ·   s  stats   ·   :  command
```

Two seconds later the sparks are gone and the screen is quiet again.

Code: `src/app/celebrate.rs` (seeded sparks, pure `frame_at(Instant)` and
`next_change_at`), `src/app/misses.rs` (per-key counts from the engine's
words, heat levels, keyboard layout), drawn in `src/ui/results.rs`.
16 unit tests between them, plus input and config tests.

---

## 2. Stats screen: a year of activity

`:stats` (or `s` on the results screen) now opens on:

- **A calendar** of tests per day, GitHub style: weeks across, Monday to
  Sunday down, month names above. Each square is shaded from the
  background towards the theme's accent by how busy the day was, and
  today is marked `▣`. Days are your local days.
- **A summary line**: your current streak (it still counts if you haven't
  typed yet today), your longest streak, total time typed and your busiest
  day.
- **A sparkline** of your average wpm for each of the last 30 days, with
  the range. Days off show as dots.

The calendar shows as many weeks as fit (up to 52). In a short terminal
it drops the calendar first and then the two lines, so the run table
always has room. The table also no longer runs into the hint line, which
it used to on long histories.

How to try: run ttyp with a `--data-dir` that has some history and press
`:stats`. To fake a year of history, use this script (it writes 600 to 700
runs, busier lately, with a holiday gap and a streak running up to today):

```python
# python3 fake-history.py /tmp/s/d/history.jsonl
import json, random, sys, datetime as dt
random.seed(11); out = open(sys.argv[1], "w")
now = dt.datetime.now().astimezone()
today = now.replace(hour=9, minute=0, second=0, microsecond=0)
if today > now: today -= dt.timedelta(hours=8)
wpm = 58.0
for back in range(330, -1, -1):
    day = today - dt.timedelta(days=back)
    p = 0.35 + 0.4 * (1 - back / 330) + (0.15 if day.weekday() >= 5 else 0)
    if 120 < back < 135: p = 0.0
    if back <= 9: p = 1.0
    if random.random() > p: continue
    wpm += 0.06
    for i in range(random.choice([1, 1, 2, 3, 4, 6, 9]) if back > 9 else random.randint(2, 8)):
        ts = day + dt.timedelta(minutes=13 * i + random.randint(0, 9))
        if ts > now: continue
        secs, w = random.choice([15, 30, 30, 60]), max(20.0, random.gauss(wpm, 6))
        out.write(json.dumps({"schema": 2, "mode": {"time": secs}, "language": "english",
            "ts": ts.astimezone(dt.timezone.utc).isoformat().replace("+00:00", "Z"),
            "punctuation": False, "numbers": False, "wpm": w, "raw": w + 4,
            "acc": random.uniform(91, 99), "consistency": random.uniform(65, 85),
            "chars": {"correct": int(w * secs / 12), "incorrect": 3, "extra": 0, "missed": 0},
            "duration_s": float(secs)}) + "\n")
```

Capture (100×34, generated history; the squares are in five shades of the
accent in the real thing):

```
          ttyp
          stats

              jan   feb     mar       apr     may     jun       jul     aug       sep
          mon ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
              ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
          wed ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
              ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ▣
          fri ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
              ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
              ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■

          streak 11 days  ·  longest 14 days  ·  typed 6h 32m  ·  best day sep 11 (9)
          last 30 days  ▃▄··▇▇▃··▆·▁·▆··▇··▃▅▃▇▄▁█▄▅▄█  60–74 wpm

          tests 688  ·  avg 64 wpm  95%  ·  last 10 73 wpm  93%
          time 15 83  ·  time 30 79  ·  time 60 81

            when         mode                language            wpm    raw    acc    con
           › 10-01 10:33  time 60             english              70     74    93%    69%
            10-01 10:24  time 30             english              77     81    95%    79%
            …
```

At 80×24 the calendar steps aside and the streak and trend lines stay. At
60×18 only the table is left.

Code: `src/stats/activity.rs` (pure: days by local date, streaks, best
day, trend, shade levels, month labels, with the time zone and "today"
passed in; 7 tests), drawn in `src/ui/stats.rs`; `ui::style::mix` blends
theme colours for the shades.

---

## 3. Pace caret

Race yourself. With `:pace pb`, a ghost caret moves through the words at
the speed of your personal best for this mode and language, starting with
your first key. It's a faint block in a soft shade of the theme's `sub`
colour *behind* the character, so the text stays readable, and your real
caret always wins when you share a cell. Ahead of it? You're on PB pace.

- `:pace pb`: your best for the mode and language
- `:pace last`: your last run in this mode and language
- `:pace 87`: a fixed 87 wpm (any 1–400)
- `:pace off`: off (the default)

Before the test starts, the mode line says what you're racing (`pace 79`,
or `pace pb (none yet)` before there's a run to race). It's also
`pace = "pb"` / `"last"` / `87` in the config, `:set pace …`, and a profile
setting, and it works in dailies (racing your best for the daily's mode
and language). Like everything else here it's only drawn: the engine,
keylog, scoring and history never see it.

Capture (100×20, `pace = "pb"` with the generated history; idle, then
mid-test; plain text can't show the shading, so the cells are marked
underneath):

```
           however take while old form what get who go change since run one turn another
           any under not see it to here then must with most say increase each can back
           who time not leave during so increase find leave know open great hold but give

           time 30  ·  english  ·  pace 79
```

```
           27

           must man very form set few we leave fact just what system face with develop
                           ^   ^
                         you   pace (79 wpm, your best)
           great out real into day because here play increase day own a on need move lead
```

Code: `src/app/pace.rs` (pure: word lengths + elapsed + wpm to word and
character, when it next moves, and the target from the history; 5 tests),
`config::Pace`, the `:pace` command, drawn in `src/ui/typing.rs`. The event
loop wakes only when the ghost moves a character, and only while a test
runs.

---

## Notes

- **Fixed:** `server = ""` used to be dropped on the first config save
  (the next start then wrote the built-in server back in, so an offline
  config quietly went online). `Config` now keeps the empty value and
  `Config::server_url` treats empty as offline; a test covers the round
  trip.
- The old agents' worktrees are still listed (`git worktree list`, under
  `~/Projects/tui-type/.claude/worktrees/`). They sit inside the main
  checkout, which this work stayed out of. Remove them with
  `git worktree remove <path>` when convenient (`agent-a1de9…` is locked).
- Docs: README has new "Results screen", "Pace caret" and "Stats screen"
  sections plus the `pace` and `results keys` command rows. CLAUDE.md
  describes the new modules.
