-- Strict one-use enforcement for interview shortcodes. hit_count stays a loose
-- counter; consumed_at is the authoritative "this link has been claimed" flag.
ALTER TABLE shortlinks ADD COLUMN consumed_at timestamptz;
