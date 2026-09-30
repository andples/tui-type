-- Every daily run the server saw begin, finished or not, so restarting
-- can't hide a bad attempt. Attempt numbers come from starts and results
-- together; a result points at its start, and only a result with one (or
-- stored before starts existed) can be a first try.
CREATE TABLE starts (
  id            INTEGER PRIMARY KEY,
  user_id       INTEGER NOT NULL REFERENCES users(id),
  daily_id      INTEGER NOT NULL REFERENCES daily_tests(id),
  attempt       INTEGER NOT NULL,
  created_at    TEXT NOT NULL,
  UNIQUE (user_id, daily_id, attempt)
);

ALTER TABLE results ADD COLUMN start_id INTEGER REFERENCES starts(id);
ALTER TABLE results ADD COLUMN first_eligible INTEGER NOT NULL DEFAULT 1;
CREATE UNIQUE INDEX results_by_start ON results (start_id) WHERE start_id IS NOT NULL;
