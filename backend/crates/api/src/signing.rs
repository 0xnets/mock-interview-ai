//! Ed25519 signing for transcript checkpoints. The key is loaded from env at
//! startup: in dev we fall back to a deterministic key derived from a seed
//! literal so the chain still verifies across restarts; prod must set
//! `TRANSCRIPT_SIGNING_SECRET_HEX` (32 bytes hex). `TRANSCRIPT_KEY_ID` lets
//! us rotate without breaking already-signed checkpoints.
//!
//! Phase 6 will replace the env-loaded key with a KMS-backed signer.

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, SECRET_KEY_LENGTH};

#[derive(Clone)]
pub struct TranscriptSigner {
    key_id: String,
    signing: SigningKey,
    verifying: VerifyingKey,
}

impl TranscriptSigner {
    pub fn from_env() -> Result<Self> {
        let key_id =
            std::env::var("TRANSCRIPT_KEY_ID").unwrap_or_else(|_| "dev-v1".to_string());
        let signing = match std::env::var("TRANSCRIPT_SIGNING_SECRET_HEX") {
            Ok(hex_secret) => {
                let bytes = hex::decode(hex_secret.trim())
                    .context("TRANSCRIPT_SIGNING_SECRET_HEX must be valid hex")?;
                if bytes.len() != SECRET_KEY_LENGTH {
                    return Err(anyhow!(
                        "TRANSCRIPT_SIGNING_SECRET_HEX must decode to {} bytes",
                        SECRET_KEY_LENGTH
                    ));
                }
                let mut arr = [0u8; SECRET_KEY_LENGTH];
                arr.copy_from_slice(&bytes);
                SigningKey::from_bytes(&arr)
            }
            Err(_) => {
                // Deterministic dev fallback. Same seed across restarts so a
                // freshly migrated DB still verifies. Never use in prod.
                tracing::warn!(
                    "TRANSCRIPT_SIGNING_SECRET_HEX not set; using insecure dev key"
                );
                let seed: [u8; SECRET_KEY_LENGTH] =
                    *b"mock-interview-dev-signing-key!!";
                SigningKey::from_bytes(&seed)
            }
        };
        let verifying = signing.verifying_key();
        Ok(Self {
            key_id,
            signing,
            verifying,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying.to_bytes()
    }

    /// Sign the canonical checkpoint payload. We sign the concatenation of
    /// the `last_row_hash`, the seq, and the key_id so a forged checkpoint
    /// can't claim a different scope.
    pub fn sign_checkpoint(&self, session_id: &str, last_seq: i32, row_hash: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + 32);
        buf.extend_from_slice(session_id.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(last_seq.to_string().as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(self.key_id.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(row_hash);
        self.signing.sign(&buf).to_bytes().to_vec()
    }
}

