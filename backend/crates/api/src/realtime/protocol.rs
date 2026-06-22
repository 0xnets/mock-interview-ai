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
        question_number: u32,
        total_questions: u32,
        kind: String,
        text: String,
    },
    /// Deliver an AI follow-up to a technical primary.
    Followup {
        ordinal: i16,
        question_number: u32,
        total_questions: u32,
        parent_ordinal: i16,
        text: String,
    },
    /// Live transcript produced server-side by the STT engine. `is_final=false`
    /// for interim text (overwrite the current segment), true when a segment is
    /// finalized (append). Lets the client render captions without doing its own
    /// speech recognition.
    Transcript {
        ordinal: i16,
        text: String,
        is_final: bool,
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
pub enum ClientMsg {
    /// Optional handshake. Currently informational.
    Hello {
        // Accepted off the wire for forward-compat; not read yet.
        #[serde(default)]
        #[allow(dead_code)]
        client_version: Option<String>,
    },
    /// Candidate started answering: begin streaming mic audio for `ordinal`.
    /// The backend opens an outbound STT session; subsequent binary WS frames
    /// are raw PCM audio for this ordinal.
    AudioStart { ordinal: i16 },
    /// Candidate stopped answering (manual stop or timer expiry): flush and
    /// terminate the STT session for `ordinal`. The actor tracks a single live
    /// STT session, so `ordinal` is accepted for protocol symmetry but not read.
    AudioEnd {
        #[allow(dead_code)]
        ordinal: i16,
    },
    /// Candidate finished the current question.
    Submit { ordinal: i16, duration_ms: i32 },
    /// Candidate skipped (allowed only on primary questions, not follow-ups).
    Skip {
        ordinal: i16,
        // Accepted off the wire but not persisted yet.
        #[serde(default)]
        #[allow(dead_code)]
        reason: Option<String>,
    },
    /// Keep-alive. Server replies with a Pong frame, not a JSON message.
    Ping,
}
