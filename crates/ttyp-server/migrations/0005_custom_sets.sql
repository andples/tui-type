-- Custom word sets players publish (2.4.0). Names are unique in any case;
-- only the owner can publish a new version or take a set down.
CREATE TABLE custom_sets (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
  owner_id    INTEGER NOT NULL REFERENCES users(id),
  words       TEXT NOT NULL,          -- JSON array
  word_count  INTEGER NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

-- Who installed which set, once each: the popularity count.
CREATE TABLE custom_installs (
  set_id      INTEGER NOT NULL REFERENCES custom_sets(id) ON DELETE CASCADE,
  user_id     INTEGER NOT NULL REFERENCES users(id),
  created_at  TEXT NOT NULL,
  PRIMARY KEY (set_id, user_id)
);
