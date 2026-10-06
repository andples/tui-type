-- One-way follows (2.2.0): `follower_id` follows `followee_id`. Following
-- needs the other profile to be public; a follow outlives the profile going
-- private, so it can still be listed and undone.
CREATE TABLE follows (
  follower_id  INTEGER NOT NULL REFERENCES users(id),
  followee_id  INTEGER NOT NULL REFERENCES users(id),
  created_at   TEXT NOT NULL,
  PRIMARY KEY (follower_id, followee_id)
);
