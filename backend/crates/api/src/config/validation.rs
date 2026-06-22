//! Configuration validation.
//!
//! Designated home for cross-field and value-range checks on loaded
//! [`Settings`]. STT settings are validated at startup because invalid channel
//! capacities would panic later and invalid provider parameters cannot recover.

use anyhow::{ensure, Result};

use super::Settings;

pub(super) fn validate(settings: &Settings) -> Result<()> {
    ensure!(
        !settings.assemblyai_streaming_url.trim().is_empty(),
        "ASSEMBLYAI_STREAMING_URL must not be empty"
    );
    ensure!(
        settings.assemblyai_sample_rate > 0,
        "ASSEMBLYAI_SAMPLE_RATE must be greater than zero"
    );
    ensure!(
        !settings.assemblyai_speech_model.trim().is_empty(),
        "ASSEMBLYAI_SPEECH_MODEL must not be empty"
    );
    ensure!(
        settings.assemblyai_audio_channel_capacity > 0,
        "ASSEMBLYAI_AUDIO_CHANNEL_CAPACITY must be greater than zero"
    );
    ensure!(
        settings.stt_event_channel_capacity > 0,
        "STT_EVENT_CHANNEL_CAPACITY must be greater than zero"
    );
    Ok(())
}
