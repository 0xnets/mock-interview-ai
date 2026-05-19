-- Phase 5: Tamper-evident transcript chain + outbox bookkeeping.
--
-- transcript_chunks already exists from 0001 (created early so DB grants could
-- be locked down on day one). This migration:
--   1. Adds the checkpoint table for signed roll-ups
--   2. Adds an attempt counter / locked_at to event_outbox so the dispatcher
--      can lease rows and back off
--   3. Adds reports.pdf_object_key was already in 0001; here we add the
--      generation status column for the outbox-driven PDF/mail pipeline

CREATE TABLE transcript_checkpoints (
    id           bigserial PRIMARY KEY,
    session_id   uuid NOT NULL REFERENCES interview_sessions(id) ON DELETE CASCADE,
    last_seq     integer NOT NULL,
    last_row_id  bigint NOT NULL REFERENCES transcript_chunks(id),
    row_hash     bytea NOT NULL,
    signature    bytea NOT NULL,
    key_id       text NOT NULL,
    chunk_count  integer NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (session_id, last_seq)
);

CREATE INDEX idx_checkpoints_session ON transcript_checkpoints(session_id, last_seq);

-- Worker lease/backoff fields. dispatched_at stays for "done"; locked_until
-- gates redelivery; attempts gives us a kill-switch threshold.
ALTER TABLE event_outbox
    ADD COLUMN attempts      integer     NOT NULL DEFAULT 0,
    ADD COLUMN locked_until  timestamptz,
    ADD COLUMN last_error    text;

CREATE INDEX idx_outbox_lease
    ON event_outbox(id)
    WHERE dispatched_at IS NULL;

-- Report generation status. Drives the outbox worker so the same row can
-- describe "queued / rendered / failed" without dropping data.
ALTER TABLE reports
    ADD COLUMN pdf_status       text NOT NULL DEFAULT 'pending'
        CHECK (pdf_status IN ('pending', 'rendered', 'failed')),
    ADD COLUMN pdf_generated_at timestamptz,
    ADD COLUMN pdf_error        text,
    ADD COLUMN mail_status      text NOT NULL DEFAULT 'pending'
        CHECK (mail_status IN ('pending', 'sent', 'failed')),
    ADD COLUMN mail_sent_at     timestamptz,
    ADD COLUMN mail_error       text;
