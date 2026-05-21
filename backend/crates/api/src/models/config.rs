//! Account-scoped HR configuration.
//!
//! Persisted as JSONB on `accounts.hr_config` and exchanged over
//! `GET/PUT /v1/me/config`. The same struct is used for the stored blob and
//! the HTTP DTO — every field has a serde default so a partially populated
//! stored blob still deserializes cleanly.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

const MIN_TECH_COUNT: u16 = 1;
const MAX_TECH_COUNT: u16 = 20;
const MAX_BEHAVIORAL_COUNT: u16 = 20;
const MIN_PASS_THRESHOLD: i16 = 0;
const MAX_PASS_THRESHOLD: i16 = 100;

const DEFAULT_TECH_COUNT: u16 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrConfig {
    #[serde(default)]
    pub hr_email: String,
    #[serde(default)]
    pub tech_count: u16,
    #[serde(default)]
    pub behavioral_count: u16,
    /// `{ "topic": ["q1", "q2"] }` (or a list of `{topic, questions}` objects).
    /// The priming worker picks one random question per topic.
    #[serde(default)]
    pub behavioral_bank: JsonValue,
    #[serde(default)]
    pub pass_threshold: i16,
    #[serde(default)]
    pub voice_name: String,
}

impl HrConfig {
    /// Config returned by `GET /v1/me/config` when an account has nothing
    /// stored yet.
    pub fn defaults(default_pass_threshold: i16) -> Self {
        Self {
            hr_email: String::new(),
            tech_count: DEFAULT_TECH_COUNT,
            behavioral_count: 0,
            behavioral_bank: json!({}),
            pass_threshold: default_pass_threshold,
            voice_name: String::new(),
        }
    }

    /// Clamp numeric fields into their accepted ranges.
    pub fn normalized(mut self) -> Self {
        self.tech_count = self.tech_count.clamp(MIN_TECH_COUNT, MAX_TECH_COUNT);
        self.behavioral_count = self.behavioral_count.min(MAX_BEHAVIORAL_COUNT);
        self.pass_threshold = self
            .pass_threshold
            .clamp(MIN_PASS_THRESHOLD, MAX_PASS_THRESHOLD);
        self
    }

    /// Validate a config that is about to be persisted or used to create an
    /// interview. Numeric ranges are enforced by [`normalized`](Self::normalized);
    /// this checks the fields that cannot simply be clamped.
    pub fn validate(&self) -> Result<(), String> {
        let email = self.hr_email.trim();
        if email.is_empty() {
            return Err("hr_email is required".into());
        }
        if !email.contains('@') {
            return Err("hr_email looks invalid".into());
        }
        match &self.behavioral_bank {
            JsonValue::Object(m) if !m.is_empty() => {}
            JsonValue::Array(a) if !a.is_empty() => {}
            _ => {
                return Err("behavioral_bank must be a non-empty object or array".into());
            }
        }
        Ok(())
    }
}

/// Response body for `GET/PUT /v1/me/config`. `saved` is false only when the
/// account has no stored config and is receiving typed defaults.
#[derive(Debug, Serialize)]
pub struct HrConfigEnvelope {
    pub config: HrConfig,
    pub saved: bool,
}
