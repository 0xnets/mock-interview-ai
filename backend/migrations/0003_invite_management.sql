-- Add stable invite identifiers and revocation metadata for admin invite
-- management. Raw invite tokens remain write-only; resend creates a new link.

ALTER TABLE account_invites
    ADD COLUMN id uuid DEFAULT gen_random_uuid(),
    ADD COLUMN revoked_at timestamptz,
    ADD COLUMN revoked_by uuid REFERENCES accounts(id);

ALTER TABLE account_invites
    ALTER COLUMN id SET NOT NULL;

ALTER TABLE account_invites
    ADD CONSTRAINT account_invites_id_key UNIQUE (id);

CREATE INDEX idx_invites_status
    ON account_invites (expires_at, consumed_at, revoked_at);

CREATE INDEX idx_invites_revoked_by
    ON account_invites (revoked_by)
    WHERE revoked_by IS NOT NULL;
