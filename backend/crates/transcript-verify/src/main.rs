//! Offline transcript verifier. Reads a transcript JSON (as returned by
//! `GET /v1/interviews/{id}/transcript`) on stdin or from a file, re-hashes
//! every chunk, and verifies each checkpoint's signature against the
//! supplied public key.
//!
//! Usage:
//!     curl ... | transcript-verify --verify
//!     transcript-verify --verify --file transcript.json
//!
//! Exit codes: 0 on success, 1 on any mismatch.

use std::io::Read;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Transcript {
    session_id: Uuid,
    public_key_b64: String,
    chunks: Vec<Chunk>,
    checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Deserialize)]
struct Chunk {
    seq: i32,
    question_id: Uuid,
    is_final: bool,
    client_ts_ms: i64,
    text: String,
    prev_hash_hex: String,
    row_hash_hex: String,
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    last_seq: i32,
    row_hash_hex: String,
    signature_b64: String,
    key_id: String,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("FAILED: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut verify = false;
    let mut file: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--verify" => verify = true,
            "--file" => file = args.next(),
            "-h" | "--help" => {
                println!("transcript-verify --verify [--file <path>]");
                return Ok(());
            }
            other => return Err(anyhow!("unknown arg: {other}")),
        }
    }
    if !verify {
        return Err(anyhow!("pass --verify (only operation supported)"));
    }

    let input = match file {
        Some(p) => std::fs::read_to_string(&p).with_context(|| format!("read {p}"))?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .context("read stdin")?;
            s
        }
    };
    let t: Transcript = serde_json::from_str(&input).context("parse transcript JSON")?;

    // Re-hash the chain.
    let mut prev = [0u8; 32];
    for c in &t.chunks {
        let prev_actual = hex::decode(&c.prev_hash_hex).context("decode prev_hash_hex")?;
        if prev_actual != prev {
            return Err(anyhow!(
                "seq {} prev_hash does not match running chain (expected {}, got {})",
                c.seq,
                hex::encode(prev),
                c.prev_hash_hex
            ));
        }
        let canonical =
            canonical_row_bytes(c.seq, c.question_id, c.is_final, c.client_ts_ms, &c.text);
        let mut h = Sha256::new();
        h.update(prev);
        h.update(&canonical);
        let computed = h.finalize();
        let row_actual = hex::decode(&c.row_hash_hex).context("decode row_hash_hex")?;
        if row_actual.as_slice() != computed.as_slice() {
            return Err(anyhow!(
                "seq {} row_hash mismatch (expected {}, got {})",
                c.seq,
                hex::encode(computed),
                c.row_hash_hex
            ));
        }
        prev.copy_from_slice(&computed);
    }

    // Verify each checkpoint signature.
    let pk_bytes = B64
        .decode(&t.public_key_b64)
        .context("decode public_key_b64")?;
    if pk_bytes.len() != 32 {
        return Err(anyhow!("public_key_b64 must decode to 32 bytes"));
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk_bytes);
    let pk = VerifyingKey::from_bytes(&pk_arr).context("parse public key")?;

    let session_id_str = t.session_id.to_string();
    for cp in &t.checkpoints {
        let row_hash = hex::decode(&cp.row_hash_hex).context("decode checkpoint row_hash_hex")?;
        let sig_bytes = B64
            .decode(&cp.signature_b64)
            .context("decode signature_b64")?;
        if sig_bytes.len() != 64 {
            return Err(anyhow!("signature must decode to 64 bytes"));
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let sig = Signature::from_bytes(&sig_arr);

        let mut buf = Vec::with_capacity(128);
        buf.extend_from_slice(session_id_str.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(cp.last_seq.to_string().as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(cp.key_id.as_bytes());
        buf.push(b'|');
        buf.extend_from_slice(&row_hash);

        pk.verify(&buf, &sig)
            .with_context(|| format!("checkpoint at last_seq={} signature invalid", cp.last_seq))?;
    }

    println!(
        "VERIFIED ({} checkpoints, {} chunks)",
        t.checkpoints.len(),
        t.chunks.len()
    );
    Ok(())
}

/// Mirror of `persistence::repo_transcript::canonical_row_bytes`. Kept in
/// sync by convention; the API ships the format string in
/// `canonical_row_format` so reviewers can diff.
fn canonical_row_bytes(
    seq: i32,
    question_id: Uuid,
    is_final: bool,
    client_ts_ms: i64,
    text: &str,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(text.len() + 96);
    buf.extend_from_slice(seq.to_string().as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(question_id.to_string().as_bytes());
    buf.push(b'|');
    buf.push(if is_final { b'1' } else { b'0' });
    buf.push(b'|');
    buf.extend_from_slice(client_ts_ms.to_string().as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(text.as_bytes());
    buf.push(b'\n');
    buf
}
