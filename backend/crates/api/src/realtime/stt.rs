//! Outbound streaming speech-to-text client for AssemblyAI Universal-Streaming.
//!
//! The session actor proxies candidate microphone audio to AssemblyAI: it opens
//! one WebSocket per answer turn, forwards raw PCM (16-bit mono) frames, and
//! receives `Turn` transcript events which are mapped to [`TranscriptEvent`]s and
//! sent back to the actor. The actor never touches the AssemblyAI socket
//! directly — it only pushes audio in and reads transcripts out through the
//! channels held by [`SttHandle`].
//!
//! Protocol reference: <https://www.assemblyai.com/docs/speech-to-text/universal-streaming>

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::config::Settings;

const TERMINATE_MSG: &str = r#"{"type":"Terminate"}"#;

/// A transcript fragment produced by the STT engine, tagged with the ordinal of
/// the question being answered so the actor routes it to the right answer.
#[derive(Debug, Clone)]
pub struct TranscriptEvent {
    pub ordinal: i16,
    pub text: String,
    pub is_final: bool,
}

/// Handle to one in-flight STT session. Dropping it (or calling [`close`]) closes
/// the audio channel, which makes the background task send `Terminate`, drain any
/// trailing transcripts, and exit. The task is detached: trailing transcripts
/// still arrive on the actor's event receiver after the handle is gone.
pub struct SttHandle {
    audio_tx: mpsc::Sender<Vec<u8>>,
}

impl SttHandle {
    /// Forward a chunk of raw PCM audio to AssemblyAI. Best-effort: if the
    /// session has ended or the buffer is saturated the frame is dropped rather
    /// than blocking the actor loop.
    pub async fn send_audio(&self, bytes: Vec<u8>) {
        let _ = self.audio_tx.send(bytes).await;
    }

    /// Signal end of audio. Drops the audio sender so the background task flushes
    /// and terminates; trailing transcripts keep flowing to the actor.
    pub fn close(self) {
        // Dropping `self` drops `audio_tx`.
    }
}

/// Open an AssemblyAI streaming session. `events_tx` receives transcript events
/// for the lifetime of the session.
pub async fn connect(
    cfg: &Settings,
    ordinal: i16,
    events_tx: mpsc::Sender<TranscriptEvent>,
) -> Result<SttHandle> {
    let api_key = cfg.assemblyai_api_key.clone();

    let url = format!(
        "{}?sample_rate={}&speech_model={}&format_turns={}",
        cfg.assemblyai_streaming_url,
        cfg.assemblyai_sample_rate,
        cfg.assemblyai_speech_model,
        cfg.assemblyai_format_turns,
    );
    let mut request = url
        .into_client_request()
        .context("building AssemblyAI streaming request")?;
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&api_key)
            .context("ASSEMBLYAI_API_KEY is not a valid header value")?,
    );

    let (ws_stream, _resp) = tokio_tungstenite::connect_async(request)
        .await
        .context("connecting to AssemblyAI streaming endpoint")?;

    let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>(cfg.assemblyai_audio_channel_capacity);
    tokio::spawn(pump(
        ws_stream,
        audio_rx,
        events_tx,
        ordinal,
        cfg.assemblyai_format_turns,
    ));

    Ok(SttHandle { audio_tx })
}

/// Drive one AssemblyAI session: forward audio in, emit transcripts out. Exits
/// when the audio channel closes (after sending `Terminate` and draining) or the
/// server closes the socket.
async fn pump<S>(
    ws: S,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
    events_tx: mpsc::Sender<TranscriptEvent>,
    ordinal: i16,
    format_turns: bool,
) where
    S: futures_util::Sink<WsMessage, Error = tokio_tungstenite::tungstenite::Error>
        + futures_util::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let (mut write, mut read) = ws.split();

    loop {
        tokio::select! {
            maybe_audio = audio_rx.recv() => {
                match maybe_audio {
                    Some(bytes) => {
                        if write.send(WsMessage::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        // Candidate stopped: ask AssemblyAI to flush + close, then
                        // drain trailing transcripts before exiting.
                        let _ = write.send(WsMessage::Text(TERMINATE_MSG.to_string())).await;
                        while let Some(Ok(msg)) = read.next().await {
                            if let WsMessage::Text(t) = msg {
                                if !forward_turn(&t, ordinal, format_turns, &events_tx).await {
                                    return;
                                }
                            }
                        }
                        break;
                    }
                }
            }
            maybe_msg = read.next() => {
                match maybe_msg {
                    Some(Ok(WsMessage::Text(t))) => {
                        if !forward_turn(&t, ordinal, format_turns, &events_tx).await {
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        tracing::debug!(error=%e, "assemblyai stream error; closing");
                        break;
                    }
                }
            }
        }
    }
}

/// AssemblyAI streaming inbound message. We only care about `Turn` frames;
/// `Begin`/`Termination` are ignored.
#[derive(Debug, Deserialize)]
struct StreamingMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    end_of_turn: bool,
    #[serde(default)]
    turn_is_formatted: bool,
}

/// Parse one inbound text frame and forward it as a [`TranscriptEvent`]. Returns
/// `false` if the actor's receiver has gone away (session should stop).
async fn forward_turn(
    raw: &str,
    ordinal: i16,
    format_turns: bool,
    events_tx: &mpsc::Sender<TranscriptEvent>,
) -> bool {
    let Ok(msg) = serde_json::from_str::<StreamingMsg>(raw) else {
        return true;
    };
    if msg.kind != "Turn" || msg.transcript.trim().is_empty() {
        return true;
    }
    // With formatted turns enabled, the provider emits an unformatted end frame
    // followed by a formatted one; only the latter is final to avoid duplicates.
    // Without formatting, the first end-of-turn frame is the final segment.
    let is_final = msg.end_of_turn && (!format_turns || msg.turn_is_formatted);
    events_tx
        .send(TranscriptEvent {
            ordinal,
            text: msg.transcript,
            is_final,
        })
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn formatted_turns_finalize_only_the_formatted_end_frame() {
        let (tx, mut rx) = mpsc::channel(2);
        let raw =
            r#"{"type":"Turn","transcript":"hello","end_of_turn":true,"turn_is_formatted":false}"#;

        assert!(forward_turn(raw, 1, true, &tx).await);
        assert!(!rx.recv().await.unwrap().is_final);

        let formatted =
            r#"{"type":"Turn","transcript":"Hello.","end_of_turn":true,"turn_is_formatted":true}"#;
        assert!(forward_turn(formatted, 1, true, &tx).await);
        assert!(rx.recv().await.unwrap().is_final);
    }

    #[tokio::test]
    async fn unformatted_turns_finalize_the_first_end_frame() {
        let (tx, mut rx) = mpsc::channel(1);
        let raw =
            r#"{"type":"Turn","transcript":"hello","end_of_turn":true,"turn_is_formatted":false}"#;

        assert!(forward_turn(raw, 1, false, &tx).await);
        assert!(rx.recv().await.unwrap().is_final);
    }
}
