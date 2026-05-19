/// JSDoc typedefs for backend response shapes. Pure documentation — no runtime code.

/**
 * @typedef {'hr'|'admin'|'super_admin'} Role
 */

/**
 * @typedef {Object} Principal
 * @property {string} account_id
 * @property {Role} role
 * @property {string} [display_name]
 */

/**
 * @typedef {Object} LoginResponse
 * @property {string} access_token
 * @property {number} expires_in  Seconds until the access token expires.
 * @property {string} account_id
 * @property {Role} role
 * @property {string} display_name
 */

/**
 * @typedef {LoginResponse} RefreshResponse
 */

/**
 * @typedef {Object} InviteCreateRequest
 * @property {string} email
 * @property {string} [display_name]
 * @property {'hr'|'admin'} role
 */

/**
 * @typedef {Object} InviteCreateResponse
 * @property {string} account_id
 * @property {string} token
 * @property {string} accept_url
 * @property {number} expires_in_hours
 */

/**
 * @typedef {Object} AcceptInviteRequest
 * @property {string} token
 * @property {string} password  Min 12 chars (backend rule).
 */

/**
 * @typedef {'pending'|'ready'|'failed'} ArtifactStatus
 */

/**
 * @typedef {Object} ReportStatusResponse
 * @property {ArtifactStatus} pdf_status
 * @property {ArtifactStatus} mail_status
 * @property {boolean} ready
 */

/**
 * @typedef {Object} TranscriptChunk
 * @property {number} seq
 * @property {string} ordinal_or_section
 * @property {string} canonical_row_b64
 * @property {string} chain_hash_b64
 */

/**
 * @typedef {Object} TranscriptCheckpoint
 * @property {number} seq
 * @property {string} chain_hash_b64
 * @property {string} signature_b64
 * @property {string} signed_at
 */

/**
 * @typedef {Object} TranscriptExport
 * @property {string} session_id
 * @property {string} public_key_b64
 * @property {string} canonical_row_format
 * @property {string} checkpoint_signing_format
 * @property {TranscriptChunk[]} chunks
 * @property {TranscriptCheckpoint[]} checkpoints
 */

export {};
