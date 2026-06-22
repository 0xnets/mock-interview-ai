-- Track how many times a candidate hit the link but could not start the
-- interview because their device/browser failed the pre-interview system
-- checks. When this crosses SHORTLINK_INCOMPAT_LIMIT the session is expired.
ALTER TABLE shortlinks ADD COLUMN incompat_count integer NOT NULL DEFAULT 0;
