-- Account sync (2.5.0): each player's configs and personal bests, so every
-- machine they log in on has the same ones. Private to the player.

-- One row per player: every config file and the active list, as JSON
-- (`ttyp_core::sync::Configs`). `rev` goes up by one per accepted push.
CREATE TABLE config_sync (
  user_id     INTEGER PRIMARY KEY REFERENCES users(id),
  rev         INTEGER NOT NULL,
  configs     TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

-- The fastest counted run per mode and language key, from any machine.
CREATE TABLE bests (
  user_id     INTEGER NOT NULL REFERENCES users(id),
  mode_kind   TEXT NOT NULL,
  mode_value  INTEGER NOT NULL,
  language    TEXT NOT NULL,
  wpm         REAL NOT NULL,
  updated_at  TEXT NOT NULL,
  PRIMARY KEY (user_id, mode_kind, mode_value, language)
);
