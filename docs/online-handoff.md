# Handoff: daily tests, server and leaderboards

Status: **built on branch `feat/online`, phases 1–7 committed; two steps
need the owner** (see *Handoff: what's left* below). This brief is
self-contained: read it, then `CLAUDE.md` and `README.md` at the repo root.
Decisions marked **decided** came from the project owner; don't re-litigate
them. Anything marked *open* is yours to settle and note here.

## Handoff: what's left (2026-09-29)

Everything in the phases below is implemented, tested and verified in tmux
/ with curl / with `docker compose`, except:

1. **Cloudflare tunnel (§7).** `docker-compose.yml` has the `cloudflared`
   service reading `TUNNEL_TOKEN` from the untracked `.env`. The tunnel
   itself was not created: it needs the owner's Cloudflare API token and
   the private notes in `~/Projects/ttyp-private/deploy-handoff.md`. To
   finish: follow those notes (token only in the shell env), put the tunnel
   token in `.env` (mode 0600), `docker compose up -d`, then the two
   *Verify* commands there.
2. **Live GitHub login (§4).** No OAuth App client id was available, so the
   device flow was tested against a mocked GitHub only (unit tests + a
   mocked `/user` endpoint in the server tests). To finish: create the
   OAuth App with device flow enabled, set `github_client_id` (and
   `server`) in a client config, run `:login`, then `:daily` and check the
   results line and `:leaderboard`.

Resolutions noted while building (no secrets):

- Workspace layout: the binary stays the root package (§1 note).
- Keylog timing: while recording, the engine snaps instants to whole
  milliseconds and logs the finishing `tick` of a time-mode run, so the
  server's replay reproduces the client's metrics bit for bit
  (`ttyp-core/src/test/keylog.rs`, tested both ways).
- `GET /dailies?date=YYYY-MM-DD` was added so the leaderboard can switch
  days (§5 lists only `/dailies/today`). Other days are not backfilled.
- Restarting (`tab`) during a daily leaves it; the daily's words can be
  seen before a "first try" by fetching and quitting. Server-side
  prevention (counting the first fetch) is out of scope; noted as open.
- The backup sidecar runs as root so it can write the host-mounted
  `./backups`; the server container stays unprivileged.
- `results::render_chart` takes `(raw, wpm)` series; errors aren't drawn,
  as before, so the offline results screen is unchanged.
- The profile list's summary ellipsis now falls at the column edge (two
  characters later than the old hand-computed width). Everything else in
  the profile menu is pixel-identical to before the port (diffed in tmux).

## Goal

Add optional online play to ttyp:

1. A server that generates **daily tests** at 00:00 UTC, the same words for
   everyone, random each day.
2. Clients fetch and type the daily, and the server scores and ranks it.
3. A **leaderboard screen** in the TUI that you can move through, where
   `enter` on an entry opens that run's wpm graph.
4. Groundwork (schema and notes only) for public user profiles.

Offline behaviour must not change. ttyp promises "no network" today, so all
online features are **opt-in**.

## Decisions

| topic | decision |
|---|---|
| Database | **SQLite** via `sqlx` with migrations. Keep queries portable so Postgres stays a cheap switch. |
| Accounts | **GitHub device flow.** No passwords, no names claimed by hand. |
| Ranking | **Two leaderboards per daily:** *first try* (each user's first run only) and *best* (each user's best run). Show both on one page if it fits, otherwise tabs (see §6). |
| Dailies scheduled at launch | languages `english`, `english_1k` × modes words 10/25/50/100 and time 15/30/60 = **14 per day** |
| Dailies' text | plain: no punctuation, no numbers |
| Scoring | the **server replays** the keystroke log through the shared engine; numbers from the client are never trusted |
| Extensibility | per-language/per-mode scheduling and on-request dailies must work later **without schema changes** (§3) |
| Hosting | **Self-hosted locally with Docker, with daily backups**, exposed through a **Cloudflare Tunnel** (§7). |
| Secrets | Cloudflare credentials and the tunnel token must **never** appear in any tracked file, commit, PR, issue, log line or code default (§7). *Changed 2026-09-29:* the public hostname and the GitHub OAuth client id are **not** secret any more; they're the built-in defaults in `src/config/mod.rs` so installs work out of the box. |
| Public profiles | **don't implement.** Leave the column and notes (§8). |
| Releases | don't bump the version, tag, or run `scripts/release.sh`. The owner releases. |

## 1. Workspace layout

Turn the repo into a Cargo workspace:

| crate | contents |
|---|---|
| `crates/ttyp-core` | `test/` (engine, generator, metrics, mode), language registry + assets, **API types** shared with the server (serde structs). No ratatui/crossterm. |
| `crates/ttyp` | today's binary: app, ui, gfx, config, profile, stats, command. Depends on `ttyp-core`. |
| `crates/ttyp-server` | axum + tokio + sqlx (SQLite). Depends on `ttyp-core`. |

*Resolved in phase 1:* the `ttyp` binary stays as the **root package** (with
`[workspace] members = ["crates/*"]`) instead of moving to `crates/ttyp`. The
Homebrew formula runs `cargo install --path .`, the AUR `PKGBUILD` builds
`target/release/ttyp` and `scripts/release.sh` bumps the root `Cargo.toml`;
all of those break with a virtual workspace root and none of them can be fixed
from this repo. `src/lib.rs` re-exports `ttyp_core::{language, test}` so the
client's `crate::test::…` paths are unchanged. `default-members` is the client
and core, so packaging builds don't compile the server; use `--workspace` for
checks.

Do the move first as its own commit with **no behaviour change**: `cargo
test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
must all pass before anything else lands. Keep the binary name `ttyp` and the
Homebrew formula path working (check `scripts/release.sh` for paths it
assumes).

## 2. Database schema (SQLite, sqlx migrations)

```sql
users (
  id            INTEGER PRIMARY KEY,
  github_id     INTEGER NOT NULL UNIQUE,
  github_login  TEXT NOT NULL,          -- display name, refreshed on login
  public        INTEGER NOT NULL DEFAULT 0,  -- unused until profiles ship
  created_at    TEXT NOT NULL
);

api_tokens (
  token_hash    BLOB PRIMARY KEY,       -- sha256 of a random 32-byte token
  user_id       INTEGER NOT NULL REFERENCES users(id),
  created_at    TEXT NOT NULL,
  last_used     TEXT
);

daily_schedule (                        -- what the scheduler generates daily
  language      TEXT NOT NULL,
  mode_kind     TEXT NOT NULL,          -- 'time' | 'words'
  mode_value    INTEGER NOT NULL,
  enabled       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (language, mode_kind, mode_value)
);

daily_tests (
  id            INTEGER PRIMARY KEY,
  date          TEXT NOT NULL,          -- UTC date, YYYY-MM-DD
  language      TEXT NOT NULL,
  mode_kind     TEXT NOT NULL,
  mode_value    INTEGER NOT NULL,
  words         TEXT NOT NULL,          -- JSON array; the source of truth
  seed          INTEGER NOT NULL,       -- provenance only
  source        TEXT NOT NULL,          -- 'schedule' | 'request'
  requested_by  INTEGER REFERENCES users(id),
  created_at    TEXT NOT NULL,
  UNIQUE (date, language, mode_kind, mode_value)
);

results (
  id                 INTEGER PRIMARY KEY,
  user_id            INTEGER NOT NULL REFERENCES users(id),
  daily_id           INTEGER NOT NULL REFERENCES daily_tests(id),
  attempt            INTEGER NOT NULL,  -- 1 = first try
  wpm REAL, raw REAL, acc REAL, consistency REAL,
  chars              TEXT NOT NULL,     -- JSON CharRecord
  wpm_per_second     TEXT NOT NULL,     -- JSON arrays for the graph
  raw_per_second     TEXT NOT NULL,
  errors_per_second  TEXT NOT NULL,
  keylog             BLOB NOT NULL,     -- as submitted, for re-scoring
  valid              INTEGER NOT NULL,
  created_at         TEXT NOT NULL,
  UNIQUE (user_id, daily_id, attempt)
);
```

Seed `daily_schedule` with the 14 launch rows in a migration.

- **Store the words, not only the seed.** A later change to a word list or
  the generator must never alter a past daily.
- **Time modes:** store `ceil(seconds × 350 / 60)` words, so nobody runs out.
- **Attempts:** the server assigns `attempt = previous count + 1` in the
  same transaction as the insert.

## 3. Daily generation

- A tokio task sleeps until the next 00:00 UTC, then inserts one
  `daily_tests` row per `enabled` schedule row, using `RandomGenerator::with_seed`
  from `ttyp-core` with a fresh random seed and no punctuation or numbers.
- **Idempotent.** Inserts rely on the `UNIQUE` key (`INSERT … ON CONFLICT DO
  NOTHING`). On startup, and on any request for today's dailies, backfill
  anything missing so a restart or a missed midnight heals itself.
- **Extending later (design for it now, don't build it):**
  - *Not every language:* already covered by `daily_schedule.enabled` and
    rows per language. No code change.
  - *On request:* a future `POST /dailies/request {language, mode}` inserts
    with `source = 'request'` and `requested_by`, going through the same
    generation function. Keep that function callable with any (language,
    mode) pair, not tied to the schedule loop.

## 4. Authentication: GitHub device flow

The client does the device flow itself with the public OAuth **client id**
(device flow needs no secret), then trades the GitHub token for a ttyp token:

1. The client posts `client_id` + `scope=read:user` to
   `https://github.com/login/device/code` and shows the `user_code` and
   `verification_uri` in the TUI.
2. The client polls `https://github.com/login/oauth/access_token` with
   `grant_type=urn:ietf:params:oauth:grant-type:device_code`, respecting
   `interval` and `slow_down`.
3. The client sends `POST /auth/github {access_token}` to the ttyp server.
   The server calls `GET https://api.github.com/user`, upserts the user by
   `github_id`, creates a random ttyp token, stores its sha256, and returns
   the token. **The server discards the GitHub token**; the client does too.
4. The client stores the ttyp token in the data dir (`token`, mode 0600) and
   sends it as `Authorization: Bearer …`.

The owner has to create a GitHub OAuth App with device flow enabled. Take the
client id from config and never hard-code it: `github_client_id` on the client
side, and an env var on the server side if it's needed there. `:logout`
deletes the token.

## 5. API

```
GET  /dailies/today                     → [{id, date, language, mode}]
GET  /dailies/{id}                      → {id, …, words}
POST /results            (auth)         → body {daily_id, keylog}
                                          → {result_id, attempt, wpm, …, rank_first, rank_best}
GET  /leaderboard/{daily_id}?board=first|best&offset=&limit=&around=me
                                        → {rows: [{rank, user, wpm, raw, acc, consistency, result_id}], me: row?}
GET  /results/{id}                      → per-second series + headline stats (for the graph)
POST /auth/github                       → {token}
```

- **Replay scoring.** A keylog is a list of `(ms_since_start, key)` where
  `key` is a char, `Backspace` or `DeleteWord`. The server builds a
  `TestEngine` over the daily's words and replays with the existing `*_at`
  methods, which already take an `Instant`, then computes `Metrics`. Reject
  (store `valid = 0`) when:
  - timings are impossible (e.g. under 10 ms between keys for long runs),
  - the duration doesn't match the mode,
  - the log doesn't finish the test.
- **Required test:** a keylog replayed on the server gives exactly the
  metrics the client showed.
- **Leaderboard queries:**
  - *first* = rows with `attempt = 1`
  - *best* = each user's max-wpm valid row
  - Order by wpm desc, then acc desc, then earliest `created_at`.
- Rate-limit submissions per user (e.g. 30 per daily per day).

## 6. Client

Config keys, all optional with online features off by default:
`server = "https://…"` and `github_client_id = "…"`.

- **Networking.** Use a blocking HTTP client (`ureq`) on a background
  thread. Replies come back to the event loop over a channel as new `Action`
  variants: `Action` was designed for remote events, and every state change
  still goes through `App::dispatch`. Never block rendering.
- **New commands** (follow the "Adding a command" convention in `CLAUDE.md`):
  - `:daily [mode]` opens today's daily for your current language, or picks
    the mode from the palette.
  - `:leaderboard` (alias `lb`)
  - `:login` and `:logout`
- **Daily runs.** Add a `FixedGenerator` implementing `WordGenerator` over
  the daily's words. The engine must record a keylog during a daily run: add
  it in `ttyp-core`, used only for dailies.
- **Results screen** for a daily: `daily · first try · #12 first · #30 best`.
  - If submission fails, queue it in the data dir and retry on the next
    start. It still counts only if accepted before the daily's UTC day ends.
  - Local `history.jsonl` records the run too, with a new optional
    `daily_id` field; bump `TestRecord::SCHEMA` and keep old lines readable.

### Leaderboard screen

It should look and feel like the profile menu (`src/ui/profiles.rs`), which
the owner likes. Aim for both boards on one page:

```
leaderboard  english · time 30 · 2026-09-29            ←→ mode · l language · [ ] day

  first try                              best
  #  name        wpm   acc   con         #  name        wpm   acc   con
› 1  sprinter    142  98.1%  81%         1  quietkeys   151  99.0%  86%
  2  quietkeys   138  99.0%  84%         2  sprinter    147  98.4%  83%
  3  andples     131  97.4%  79%         …
  …
 12  you         104  96.0%  72%        30  you         118  97.0%  75%

enter graph · ↑↓ move · tab switch board · esc back
```

- **Width < ~110 columns:** show one board at a time, with `tab` switching.
  Otherwise side by side, and `tab` moves the selection between boards.
- **Keys:**
  - `↑↓`/`jk` move, `g`/`G` top/bottom, `pgup`/`pgdn` page
  - `←→` mode, `l` language, `[` `]` previous/next day
  - `esc` back
- Your own row is always visible (pinned at the bottom when off-screen) and
  highlighted.
- Load 50 rows plus your row, and fetch the next page when the selection
  nears the end. States: `loading…`, `offline: set server in config`,
  `no results yet`.
- **Enter opens the graph view:** the wpm/raw per-second chart plus headline
  stats for that run, from `GET /results/{id}`. Refactor
  `ui::results::render_chart` to take plain series (`&[f64]`, errors) instead
  of `&Outcome` so the results screen and this view share it. `esc` returns
  with the same row selected.

### Reusable table/list framework (build this)

The owner asked for a small framework so tabular screens are easy from now
on. ratatui already ships `Table` + `TableState` (selection, scrolling), so
build **thin, themed pieces on top of it** rather than a new widget system:

- **`ttyp-core` or `src/ui/` state (no rendering):** `Selection { selected,
  offset, len }` with `move_by`, `page`, `home`, `end`, `ensure_visible(height)`,
  wrap or clamp. It must be unit-tested.
- **`src/ui/widgets/table.rs`:** `SelectTable` takes column specs (header,
  width or min width, alignment, truncation with `…`), rows of cells with a
  style role (normal / dim / accent / error), a selection, a focus flag, and
  an optional pinned row. It renders with `Palette` and the `›` marker used
  elsewhere.
- **`src/ui/widgets/panes.rs`:** split a content column into N side-by-side
  panes when wide enough, otherwise report "tabs" so the caller renders one
  pane and a tab strip.
- **`src/ui/widgets/hints.rs`:** the bottom hint line, which is currently
  copied between screens.

Then **port the profile list** (`src/ui/profiles.rs`, `src/profile/menu.rs`)
onto `Selection` + `SelectTable` as proof. It must look and behave the same
(check it in tmux). Add a line to `CLAUDE.md` under Conventions describing
how to build a new list screen.

## 7. Server deployment (decided: local Docker, daily backups)

The owner hosts the server on their own machine with Docker.

**Image.** A multi-stage `crates/ttyp-server/Dockerfile`: build with the Rust
image, run on `debian:stable-slim` (or distroless) as a non-root user. Include
the `sqlite3` CLI in the runtime image for backups and inspection.

**Compose.** A `docker-compose.yml` at the repo root:

- service `ttyp-server`, `restart: unless-stopped`, port `8080` (configurable)
- named volume `ttyp-data` mounted at `/data`, with the DB at `/data/ttyp.db`
- env vars: `TTYP_BIND=0.0.0.0:8080`, `TTYP_DB=/data/ttyp.db`, `RUST_LOG`
- a healthcheck on `GET /health` (add this endpoint; it returns the DB status)

The server runs migrations on startup and opens SQLite in WAL mode with
`busy_timeout` set.

**Daily backups.**

- **Method.** Use SQLite's online backup, `sqlite3 /data/ttyp.db ".backup
  /backups/ttyp-YYYY-MM-DD.db"` (or `VACUUM INTO`). Never `cp` the live file:
  with WAL that can capture a torn copy.
- **Schedule.** Run it from a small `ttyp-backup` sidecar service in the same
  compose file (same image, a loop that sleeps until 00:30 UTC, then backs
  up). 00:30 UTC lands after the 00:00 generation, so each backup includes
  that day's dailies.
- **Storage.** Write backups to a **host bind mount** (`./backups:/backups`),
  not the data volume, so they survive `docker volume rm`. Gzip them and keep
  the last 14 daily files (configurable, `TTYP_BACKUP_KEEP`). After each
  backup, run `PRAGMA integrity_check` on the copy and log the result.
- **Restore.** Document it in the server README and test it once: stop the
  server, copy a backup to `/data/ttyp.db`, remove any `-wal`/`-shm` files,
  start the server.
- **Off-machine copy (recommended to the owner, not built).** A local disk
  failure takes both the DB and the backups, so suggest syncing `./backups`
  somewhere else (another disk, NAS or cloud via rclone).

**Downtime is tolerated by design.** If the machine is off at 00:00 UTC, the
startup/on-request backfill (§3) creates the missing day's dailies when it
comes back. Nothing needs to run exactly at midnight.

**Exposure: Cloudflare Tunnel (decided).** A `cloudflared` service in the
same compose file, `cloudflare/cloudflared` image, running `tunnel
--no-autoupdate run`. It connects out to Cloudflare, so no ports are opened
on the host. The server stays plain HTTP on the compose network, and the
tunnel's public hostname routes to `http://ttyp-server:8080`. Don't publish
the server's port on the host at all, unless it's bound to `127.0.0.1` for
local debugging.

**Secrets handling. This is a hard rule.**

- The public hostname and all Cloudflare credentials come from the owner
  out-of-band. Don't write them into this document, the README, compose
  files, code, tests, commit messages or PR descriptions.
- Compose reads everything from an untracked `.env` next to
  `docker-compose.yml`:
  - `TUNNEL_TOKEN` for `cloudflared`
  - `TTYP_PUBLIC_URL` if the server needs to know its own URL
  
  Commit a `.env.example` with **placeholder values only**, and set `.env`
  to mode 0600.
- `.gitignore` already covers `.env`, `backups/` and cloudflared credential
  files. Keep it that way, and check `git status` / `git diff --cached` for
  secrets before every commit.
- The client has **no hard-coded default server.** Users set `server` in
  `config.toml`. If a built-in default is wanted later, inject it at release
  build time (`option_env!("TTYP_DEFAULT_SERVER")`) so it's never in the
  repo.
- If you're given a Cloudflare API token to create the tunnel, use it only
  in the shell environment for the API calls. Don't save it to disk. Store
  only the resulting tunnel token in `.env`.

**Deliverables.**

- the `Dockerfile` and `docker-compose.yml` (server + backup sidecar)
- `crates/ttyp-server/README.md` covering: run (`docker compose up -d`), logs,
  where the DB and backups live, restore steps, and how the tunnel is wired
  (generically: "set `TUNNEL_TOKEN` in `.env`", no hostnames)

## 8. Public profiles (notes only, don't build)

- `users.public` exists from the first migration, default private, so
  enabling profiles later needs no migration.
- Future `GET /users/{login}` would return personal bests per mode, daily
  streak and recent daily results, only when `public = 1`. The TUI would
  open it with `p` on a leaderboard row.
- Opt-in via a later `:profile public on`. Watch the naming clash with the
  existing `:profile` command for config profiles: pick something like
  `:account public on`.
- Local non-daily history never syncs unless a separate opt-in is added.

## Phases

Each phase ends green (`cargo test`, clippy with `-D warnings`, `cargo fmt
--check`) and is committed separately on a feature branch, not `main`.

1. **Workspace split**, no behaviour change.
2. **Table framework** + profile menu port (verify in tmux: `CLAUDE.md` has
   the headless recipe).
3. **Server**: migrations, generation + backfill, daily endpoints, replay
   scoring with the client/server equality test.
4. **Auth**: GitHub device flow end to end, `:login`/`:logout`.
5. **Client daily flow**: `:daily`, keylog, submission queue, results line.
6. **Leaderboard screen** + graph view.
7. **Deployment**: Dockerfile, compose with backup sidecar, `/health`,
   server README with a tested restore.

## Acceptance checklist

- [ ] Offline ttyp behaves exactly as before; no network without `server` set.
- [ ] 14 dailies appear at 00:00 UTC; restarting the server never duplicates
      or loses one; a missed midnight backfills.
- [ ] Two users typing the same daily see the same words.
- [ ] Server-computed metrics equal client-shown metrics for the same keylog.
- [ ] First-try and best boards both rank correctly (tests with seeded data).
- [ ] Leaderboard: move with keys, switch board/mode/language/day, your row
      pinned, enter opens the graph and esc returns to the same row.
- [ ] Profile menu looks identical after the port to the table framework.
- [ ] Login via GitHub device flow stores only a ttyp token (0600); no GitHub
      token persisted anywhere.
- [ ] README documents `server`, `:daily`, `:leaderboard`, `:login`.
- [ ] No hostname, token or Cloudflare identifier appears anywhere in git
      history (`git log -p | grep -i` for the hostname and token prefix
      before pushing).
- [ ] `docker compose up -d` starts the server; data survives container
      recreation; a backup appears in `./backups` daily, old ones are pruned,
      and a restore from one has been tested.

## Codebase conventions to keep (from `CLAUDE.md`)

- Every state change goes through `App::dispatch(Action)`.
- Engine methods that depend on time take an `Instant` (`*_at`), which keeps
  tests deterministic and is exactly what replay scoring needs.
- Config changes are saved immediately via `App::save_config`.
- New settings go in `Config`, `ProfileSettings` and the `settings!` macro
  (`src/profile/mod.rs`). `server` and `github_client_id` should **not** be
  profile settings.
- Match the surrounding code's comment density and naming. Run `cargo fmt`.
