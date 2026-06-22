pub mod anthropic;
pub mod prompts;

use std::time::Duration;

use async_trait::async_trait;
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("provider returned no text")]
    EmptyResponse,
    #[error("provider returned invalid JSON: {0}")]
    InvalidJson(String),
    #[error("provider returned an error: {status} {body}")]
    ProviderStatus { status: u16, body: String },
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("provider timed out after {0:?}")]
    Timeout(Duration),
    #[error("provider misconfigured: {0}")]
    Misconfigured(String),
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_tokens: u32,
    pub timeout: Duration,
    pub temperature: f32,
}

impl Budget {
    pub fn balanced() -> Self {
        Self {
            max_tokens: 2048,
            timeout: Duration::from_secs(60),
            temperature: 0.4,
        }
    }

    pub fn rubric() -> Self {
        Self {
            max_tokens: 800,
            timeout: Duration::from_secs(30),
            temperature: 0.3,
        }
    }

    pub fn scoring() -> Self {
        Self {
            max_tokens: 4096,
            timeout: Duration::from_secs(90),
            temperature: 0.2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub text: String,
    pub usage: TokenUsage,
    pub model: String,
    pub latency_ms: u32,
}

/// `AiProvider` is a `dyn`-compatible trait so the API can hold it behind
/// `Arc<dyn AiProvider>`. Callers parse JSON via [`parse_json`] separately.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;

    /// Generate a completion. `text` is the raw model output with any JSON
    /// fences already stripped by the provider impl, so it can be passed
    /// straight into `serde_json::from_str` when the caller expects JSON.
    async fn complete(
        &self,
        system_prompt: &str,
        user_message: &str,
        budget: Budget,
        model: &str,
    ) -> Result<Completion, AiError>;
}

/// Convenience helper for JSON-typed callers.
pub fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T, AiError> {
    serde_json::from_str::<T>(text).map_err(|e| {
        let preview = if text.len() > 400 { &text[..400] } else { text };
        AiError::InvalidJson(format!("{e}; payload={preview}"))
    })
}
