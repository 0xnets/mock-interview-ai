-- Production simplification: drop dead surface area now that the PDF/S3 path
-- is replaced by on-demand render + Resend attachment.

-- interview_configs was never written or read; sessions store their config as
-- jsonb in `config_snapshot`. Drop the table and the dangling FK.
ALTER TABLE interview_sessions DROP COLUMN IF EXISTS config_id;
DROP TABLE IF EXISTS interview_configs;

-- candidate_email was write-only (mail is sent to hr_email, never to candidate).
ALTER TABLE interview_sessions DROP COLUMN IF EXISTS candidate_email;

-- audio_object_key on answers was never written or read; no server-side audio
-- upload exists in the live flow.
ALTER TABLE answers DROP COLUMN IF EXISTS audio_object_key;

-- The PDF upload state machine is gone — PDFs render on demand and ship as
-- email attachments.
ALTER TABLE reports DROP COLUMN IF EXISTS pdf_object_key;
ALTER TABLE reports DROP COLUMN IF EXISTS pdf_status;
ALTER TABLE reports DROP COLUMN IF EXISTS pdf_generated_at;
ALTER TABLE reports DROP COLUMN IF EXISTS pdf_error;
