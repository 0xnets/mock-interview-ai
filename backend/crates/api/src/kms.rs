//! Phase 6: KMS-backed signer abstraction for transcript checkpoints.
//!
//! The previous `TranscriptSigner` loaded an Ed25519 secret from env. Phase 6
//! introduces this trait so production deploys can plug in AWS KMS / GCP KMS
//! / Vault Transit without touching the realtime actor.
//!
//! `KMS_PROVIDER` selects the implementation:
//!   * `env`   — Ed25519 secret from `TRANSCRIPT_SIGNING_SECRET_HEX`
//!   * `aws`   — AWS KMS asymmetric sign (Ed25519 / RSASSA-PSS — TODO)
//!   * `gcp`   — Google Cloud KMS (TODO)
//!   * `vault` — HashiCorp Vault Transit (TODO)
//!
//! Cloud providers are stubbed today; the trait + factory wiring lets us add
//! one without touching the actor or the verifier protocol.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::config::Settings;
use crate::signing::TranscriptSigner;

#[async_trait]
pub trait CheckpointSigner: Send + Sync {
    fn key_id(&self) -> &str;
    fn public_key_bytes(&self) -> [u8; 32];
    /// Same canonical payload as `TranscriptSigner::sign_checkpoint` —
    /// `session_id|last_seq|key_id|row_hash`.
    async fn sign_checkpoint(
        &self,
        session_id: &str,
        last_seq: i32,
        row_hash: &[u8],
    ) -> Result<Vec<u8>>;
}

/// Adapter so the env-loaded ed25519 signer satisfies the new trait.
pub struct EnvSigner(pub Arc<TranscriptSigner>);

#[async_trait]
impl CheckpointSigner for EnvSigner {
    fn key_id(&self) -> &str {
        self.0.key_id()
    }
    fn public_key_bytes(&self) -> [u8; 32] {
        self.0.public_key_bytes()
    }
    async fn sign_checkpoint(
        &self,
        session_id: &str,
        last_seq: i32,
        row_hash: &[u8],
    ) -> Result<Vec<u8>> {
        Ok(self.0.sign_checkpoint(session_id, last_seq, row_hash))
    }
}

pub fn build_signer(cfg: &Settings, env_signer: Arc<TranscriptSigner>) -> Result<Arc<dyn CheckpointSigner>> {
    match cfg.kms_provider.as_str() {
        "env" => Ok(Arc::new(EnvSigner(env_signer))),
        "aws" => Err(anyhow!(
            "KMS_PROVIDER=aws is not yet implemented; fall back to env with FEATURE rollback"
        )),
        "gcp" => Err(anyhow!("KMS_PROVIDER=gcp is not yet implemented")),
        "vault" => Err(anyhow!("KMS_PROVIDER=vault is not yet implemented")),
        other => Err(anyhow!("unknown KMS_PROVIDER={other}")),
    }
}
