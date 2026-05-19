-- Phase 2 placeholder: a single shared HR account so `interview_sessions.account_id`
-- has something to reference. Phase 3 (admin-invited auth) replaces this with
-- real accounts. The fixed UUID matches the Settings default so the API crate
-- can attribute every session created via the dev bearer token to this row.

INSERT INTO accounts (id, email, role, status, display_name)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'default-hr@local',
    'hr',
    'active',
    'Default HR (dev)'
)
ON CONFLICT (id) DO NOTHING;
