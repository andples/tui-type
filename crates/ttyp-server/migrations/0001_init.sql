-- Schema from docs/online-handoff.md §2. Kept portable (no SQLite-only
-- types) so Postgres stays a cheap switch.

CREATE TABLE users (
  id            INTEGER PRIMARY KEY,
  github_id     INTEGER NOT NULL UNIQUE,
  github_login  TEXT NOT NULL,
  public        INTEGER NOT NULL DEFAULT 0,
  created_at    TEXT NOT NULL
);

CREATE TABLE api_tokens (
  token_hash    BLOB PRIMARY KEY,
  user_id       INTEGER NOT NULL REFERENCES users(id),
  created_at    TEXT NOT NULL,
  last_used     TEXT
);

CREATE TABLE daily_schedule (
  language      TEXT NOT NULL,
  mode_kind     TEXT NOT NULL,
  mode_value    INTEGER NOT NULL,
  enabled       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (language, mode_kind, mode_value)
);

CREATE TABLE daily_tests (
  id            INTEGER PRIMARY KEY,
  date          TEXT NOT NULL,
  language      TEXT NOT NULL,
  mode_kind     TEXT NOT NULL,
  mode_value    INTEGER NOT NULL,
  words         TEXT NOT NULL,
  seed          INTEGER NOT NULL,
  source        TEXT NOT NULL,
  requested_by  INTEGER REFERENCES users(id),
  created_at    TEXT NOT NULL,
  UNIQUE (date, language, mode_kind, mode_value)
);

CREATE TABLE results (
  id                 INTEGER PRIMARY KEY,
  user_id            INTEGER NOT NULL REFERENCES users(id),
  daily_id           INTEGER NOT NULL REFERENCES daily_tests(id),
  attempt            INTEGER NOT NULL,
  wpm REAL, raw REAL, acc REAL, consistency REAL,
  chars              TEXT NOT NULL,
  wpm_per_second     TEXT NOT NULL,
  raw_per_second     TEXT NOT NULL,
  errors_per_second  TEXT NOT NULL,
  keylog             BLOB NOT NULL,
  valid              INTEGER NOT NULL,
  created_at         TEXT NOT NULL,
  UNIQUE (user_id, daily_id, attempt)
);

CREATE INDEX results_by_daily ON results (daily_id, valid, wpm);

-- The 14 launch dailies: two languages × words 10/25/50/100 × time 15/30/60.
INSERT INTO daily_schedule (language, mode_kind, mode_value) VALUES
  ('english', 'words', 10), ('english', 'words', 25),
  ('english', 'words', 50), ('english', 'words', 100),
  ('english', 'time', 15), ('english', 'time', 30), ('english', 'time', 60),
  ('english_1k', 'words', 10), ('english_1k', 'words', 25),
  ('english_1k', 'words', 50), ('english_1k', 'words', 100),
  ('english_1k', 'time', 15), ('english_1k', 'time', 30), ('english_1k', 'time', 60);
