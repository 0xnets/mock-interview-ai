use std::time::Instant;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

use crate::{AiError, AiProvider, Budget, Completion, TokenUsage};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    http: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Result<Self, AiError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(AiError::Misconfigured(
                "ANTHROPIC_API_KEY is empty".to_string(),
            ));
        }
        let http = Client::builder().pool_max_idle_per_host(8).build()?;
        Ok(Self {
            api_key,
            base_url: DEFAULT_BASE_URL.to_string(),
            http,
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn complete(
        &self,
        system_prompt: &str,
        user_message: &str,
        budget: Budget,
        model: &str,
    ) -> Result<Completion, AiError> {
        let req = MessagesRequest {
            model,
            max_tokens: budget.max_tokens,
            temperature: Some(budget.temperature),
            system: system_prompt,
            messages: vec![Message {
                role: "user",
                content: user_message,
            }],
        };

        let started = Instant::now();
        let pending = self
            .http
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&req)
            .send();

        let resp = timeout(budget.timeout, pending)
            .await
            .map_err(|_| AiError::Timeout(budget.timeout))??;
        let status = resp.status();
        let body_text = resp.text().await?;
        let elapsed = started.elapsed();

        if !status.is_success() {
            return Err(AiError::ProviderStatus {
                status: status.as_u16(),
                body: truncate(&body_text, 800),
            });
        }
        if status == StatusCode::NO_CONTENT {
            return Err(AiError::EmptyResponse);
        }

        let parsed: MessagesResponse = serde_json::from_str(&body_text).map_err(|e| {
            AiError::InvalidJson(format!("envelope: {e}; body={}", truncate(&body_text, 400)))
        })?;
        let text_full = parsed
            .content
            .into_iter()
            .filter_map(|b| if b.kind == "text" { Some(b.text) } else { None })
            .collect::<Vec<_>>()
            .join("");

        if text_full.trim().is_empty() {
            return Err(AiError::EmptyResponse);
        }

        let text = strip_json_envelope(&text_full).to_string();

        let usage = TokenUsage {
            input_tokens: parsed.usage.as_ref().map(|u| u.input_tokens).unwrap_or(0),
            output_tokens: parsed.usage.as_ref().map(|u| u.output_tokens).unwrap_or(0),
        };

        Ok(Completion {
            text,
            usage,
            model: parsed.model.unwrap_or_else(|| model.to_string()),
            latency_ms: elapsed.as_millis().min(u32::MAX as u128) as u32,
        })
    }
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    system: &'a str,
    messages: Vec<Message<'a>>,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    model: Option<String>,
    content: Vec<ContentBlock>,
    #[serde(default)]
    usage: Option<UsageBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct UsageBlock {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut t = s[..n].to_string();
        t.push_str("…");
        t
    }
}

/// Some models wrap JSON in ```json … ``` fences or include a preamble.
/// Strip the wrapper so serde_json sees pure JSON.
pub(crate) fn strip_json_envelope(raw: &str) -> &str {
    let trimmed = raw.trim();

    if let Some(fenced) = trimmed.strip_prefix("```json") {
        let after = fenced.trim_start_matches('\n').trim_start_matches('\r');
        if let Some(end) = after.rfind("```") {
            return after[..end].trim();
        }
        return after.trim();
    }
    if let Some(fenced) = trimmed.strip_prefix("```") {
        let after = fenced.trim_start_matches('\n').trim_start_matches('\r');
        if let Some(end) = after.rfind("```") {
            return after[..end].trim();
        }
        return after.trim();
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::strip_json_envelope;

    #[test]
    fn strips_json_fences() {
        let raw = "```json\n{\"a\":1}\n```";
        assert_eq!(strip_json_envelope(raw), "{\"a\":1}");
    }

    #[test]
    fn passes_through_bare_json() {
        let raw = "  {\"a\":1}  ";
        assert_eq!(strip_json_envelope(raw), "{\"a\":1}");
    }
}
