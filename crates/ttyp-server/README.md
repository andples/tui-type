# ttyp-server

The online half of ttyp: generates the daily tests at 00:00 UTC, scores
submitted keystroke logs by replaying them through `ttyp-core`, and serves
the leaderboards. SQLite, one binary, no external services besides GitHub
for login.

## Run with Docker (recommended)

From the repo root:

```sh
cp .env.example .env && chmod 600 .env   # then fill in TUNNEL_TOKEN
docker compose up -d --build
docker compose ps                        # ttyp-server should be "healthy"
docker compose logs -f ttyp-server
```

Services:

| service | what |
|---|---|
| `ttyp-server` | the API on port 8080 inside the compose network; not published on the host |
| `ttyp-backup` | same image; backs the database up every day at 00:30 UTC |
| `cloudflared` | Cloudflare Tunnel; connects out, so no inbound ports are opened |

The server runs its migrations on start, opens SQLite in WAL mode with a
busy timeout, and creates any missing dailies for today on start and
whenever today's list is requested, so a machine that was off at midnight
heals itself when it comes back.

### Where things live

| | path |
|---|---|
| database | named volume `ttyp-data`, mounted at `/data/ttyp.db` |
| backups | `./backups/ttyp-YYYY-MM-DD.db.gz` on the host (bind mount) |
| secrets | `.env` next to `docker-compose.yml` (git-ignored) |

Inspect the live database with the bundled CLI:

```sh
docker compose exec ttyp-server sqlite3 /data/ttyp.db \
  'select date, count(*) from daily_tests group by date'
```

### Backups

`ttyp-backup` uses SQLite's online backup (`.backup`), never a file copy,
runs `PRAGMA integrity_check` on the copy, gzips it and keeps the newest
`TTYP_BACKUP_KEEP` (default 14) files. 00:30 UTC is after the 00:00
generation, so each backup includes that day's dailies. The sidecar runs
as root so it can write to `./backups` on the host (the server does not);
the files it leaves there are root-owned and world-readable. To back up
right now:

```sh
docker compose run --rm ttyp-backup ttyp-backup once
```

A disk failure takes the volume and `./backups` together. Sync `./backups`
somewhere else too (another disk, a NAS, or cloud via rclone).

### Restore

1. Stop the server: `docker compose stop ttyp-server ttyp-backup`
2. Unpack the chosen backup over the live database, drop the WAL/shm files
   (they belong to the old database) and give the file back to the server's
   user, all inside the volume:
   ```sh
   docker compose run --rm --no-deps --user 0 --entrypoint sh \
     -v "$PWD/backups:/backups:ro" ttyp-server -c '
       gunzip -c /backups/ttyp-2026-09-29.db.gz > /data/ttyp.db &&
       rm -f /data/ttyp.db-wal /data/ttyp.db-shm &&
       chown ttyp:ttyp /data/ttyp.db'
   ```
3. `docker compose start ttyp-server ttyp-backup`, then check that
   `docker compose ps` shows `ttyp-server` healthy.

### The tunnel

`cloudflared` runs `tunnel --no-autoupdate run` with `TUNNEL_TOKEN` from
`.env`. In Cloudflare, the tunnel's ingress routes your public hostname to
`http://ttyp-server:8080`; TLS terminates at Cloudflare. Clients then set
`server = "https://<your hostname>"` in their ttyp config. Verify with:

```sh
curl -fsS https://<your hostname>/health
docker compose logs cloudflared | grep -i "registered tunnel connection"
```

Nothing in this repo names the hostname or holds credentials; keep it that way.

## Run without Docker

```sh
TTYP_BIND=127.0.0.1:8080 TTYP_DB=./ttyp.db RUST_LOG=info cargo run -p ttyp-server
```

| variable | default | |
|---|---|---|
| `TTYP_BIND` | `127.0.0.1:8080` | listen address |
| `TTYP_DB` | `ttyp.db` | SQLite file, created if missing |
| `RUST_LOG` | `info` | tracing filter |
| `TTYP_GITHUB_API` | `https://api.github.com` | overridden by tests to mock GitHub |

`ttyp-server healthcheck` performs `GET /health` against the local port and
exits non-zero if it fails (used as the container healthcheck).

## API

```
GET  /health                              {"status":"ok","dailies_today":14}
GET  /dailies/today                       [{id, date, language, mode}]
GET  /dailies?date=YYYY-MM-DD             same, for another UTC day
GET  /dailies/{id}                        {id, date, language, mode, words}
POST /results            (bearer)         {daily_id, keylog} → {result_id, attempt, valid, wpm, …, rank_first, rank_best}
GET  /results/{id}                        per-second series and headline stats
GET  /leaderboard/{daily_id}?board=first|best&offset=&limit=&around=me
POST /auth/github                         {access_token} → {token, login}
POST /auth/logout        (bearer)         revoke the token
```

Keylogs are `[{ms, key}]` where `key` is a character, `backspace`,
`delete_word` or `tick` (the timer tick that ended a time-mode run). The
server replays the log through the same engine the client uses and only
trusts the metrics it computes itself; runs that don't finish, don't match
the mode's duration, or are faster than a human are stored as invalid and
never ranked. Each user gets at most 30 submissions per daily.

Bearer tokens are 32 random bytes; only their SHA-256 is stored. The GitHub
access token is used for one `GET /user` call and discarded.
