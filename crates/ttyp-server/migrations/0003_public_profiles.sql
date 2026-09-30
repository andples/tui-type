-- Profiles are public by default (owner's decision, 2026-10-01): new users
-- are created with public = 1, and existing ones, who never chose, follow.
-- `:account public off` still hides a profile.
UPDATE users SET public = 1;
