-- Account-scoped HR configuration.
-- Stores the reusable HR setup (recipient email, question counts, behavioral
-- bank, pass threshold, interviewer voice) previously kept in browser
-- localStorage. Nullable so existing accounts rows remain valid; populated via
-- PUT /v1/me/config.
ALTER TABLE accounts ADD COLUMN hr_config jsonb;
