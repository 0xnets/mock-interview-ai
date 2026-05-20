//! WebSocket message envelopes. JSON over Text frames per the plan §6.3.
//!
//! Server → client uses [`ServerMsg`], client → server uses [`ClientMsg`].
//! `serde`'s tagged enum encoding gives the `{"t": "..."}` envelope.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Backend booted/attached. Used by the client to know it's safe to render.
    Hello {
        session_id: String,
        candidate_name: String,
        role_title: String,
        state: String,
        total_questions: u32,
    },
    /// Deliver the next primary question (intro/technical/behavioral).
    Question {
        ordinal: i16,
        kind: String,
        text: String,
    },
    /// Deliver an AI follow-up to a technical primary.
    Followup {
        ordinal: i16,
        parent_ordinal: i16,
        text: String,
    },
    /// State machine transition or transient indicator (e.g. "thinking").
    State { value: String },
    /// Last submit succeeded; the candidate should now hit POST /finalize.
    ResultsReady { session_id: String },
    /// Recoverable or terminal error; `terminal=true` means we will close.
    Error {
        code: String,
        message: String,
        #[serde(default)]
        terminal: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ClientMsg {
    /// Optional handshake. Currently informational.
    Hello {
        #[serde(default)]
        client_version: Option<String>,
    },
    /// Streaming STT chunk. `is_final=false` for interim, true for final.
    /// Phase 4 stores them in memory only; Phase 5 will hash-chain to DB.
    Utterance {
        seq: u32,
        ordinal: i16,
        text: String,
        is_final: bool,
    },
    /// Candidate finished the current question.
    Submit { ordinal: i16, duration_ms: i32 },
    /// Candidate skipped (allowed only on primary questions, not follow-ups).
    Skip {
        ordinal: i16,
        #[serde(default)]
        reason: Option<String>,
    },
    /// Keep-alive. Server replies with a Pong frame, not a JSON message.
    Ping,
}
