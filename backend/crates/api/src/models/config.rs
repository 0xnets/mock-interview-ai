//! Account-scoped HR configuration.
//!
//! Persisted as JSONB on `accounts.hr_config` and exchanged over
//! `GET/PUT /v1/me/config`. The same struct is used for the stored blob and
//! the HTTP DTO — every field has a serde default so a partially populated
//! stored blob still deserializes cleanly.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use crate::{config::defaults, validation::validate_email};

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
            tech_count: defaults::DEFAULT_TECH_COUNT,
            behavioral_count: 0,
            behavioral_bank: json!({}),
            pass_threshold: default_pass_threshold,
            voice_name: String::new(),
        }
    }

    /// Clamp numeric fields into their accepted ranges.
    pub fn normalized(mut self) -> Self {
        self.tech_count = self
            .tech_count
            .clamp(defaults::MIN_TECH_COUNT, defaults::MAX_TECH_COUNT);
        self.behavioral_count = self.behavioral_count.min(defaults::MAX_BEHAVIORAL_COUNT);
        self.pass_threshold = self
            .pass_threshold
            .clamp(defaults::MIN_PASS_THRESHOLD, defaults::MAX_PASS_THRESHOLD);
        self
    }

    /// Validate a config that is about to be persisted or used to create an
    /// interview. Numeric ranges are enforced by [`normalized`](Self::normalized);
    /// this checks the fields that cannot simply be clamped.
    pub fn validate(&self) -> Result<(), String> {
        let email = self.hr_email.trim();
        validate_email(email, "hr_email")?;

        let stats = validate_behavioral_bank(&self.behavioral_bank)?;
        if self.behavioral_count as usize > stats.populated_sections {
            return Err(format!(
                "behavioral_count requires {} populated sections, but only {} are available",
                self.behavioral_count, stats.populated_sections
            ));
        }
        Ok(())
    }
}

#[derive(Default)]
struct BankStats {
    populated_sections: usize,
    total_questions: usize,
}

fn validate_behavioral_bank(bank: &JsonValue) -> Result<BankStats, String> {
    let stats = match bank {
        JsonValue::Object(sections) => validate_bank_object(sections)?,
        JsonValue::Array(sections) => validate_bank_array(sections)?,
        _ => return Err("behavioral_bank must be a non-empty object or array".into()),
    };

    if stats.populated_sections == 0 {
        return Err("behavioral_bank must include at least one populated section".into());
    }
    if stats.populated_sections > defaults::MAX_BANK_SECTIONS {
        return Err(format!(
            "behavioral_bank has too many sections; max is {}",
            defaults::MAX_BANK_SECTIONS
        ));
    }
    if stats.total_questions > defaults::MAX_BANK_QUESTIONS {
        return Err(format!(
            "behavioral_bank has too many questions; max is {}",
            defaults::MAX_BANK_QUESTIONS
        ));
    }
    Ok(stats)
}

fn validate_bank_object(
    sections: &serde_json::Map<String, JsonValue>,
) -> Result<BankStats, String> {
    let mut stats = BankStats::default();
    for (section, questions) in sections {
        validate_section_name(section)?;
        let questions = questions
            .as_array()
            .ok_or_else(|| format!("behavioral_bank section '{section}' must be an array"))?;
        validate_questions(section, questions, &mut stats)?;
    }
    Ok(stats)
}

fn validate_bank_array(sections: &[JsonValue]) -> Result<BankStats, String> {
    let mut stats = BankStats::default();
    for (idx, section_value) in sections.iter().enumerate() {
        let section = section_value
            .as_object()
            .ok_or_else(|| format!("behavioral_bank item {} must be an object", idx + 1))?;
        let topic = section
            .get("topic")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| format!("behavioral_bank item {} needs a topic", idx + 1))?;
        validate_section_name(topic)?;
        let questions = section
            .get("questions")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| format!("behavioral_bank item '{}' needs questions", topic))?;
        validate_questions(topic, questions, &mut stats)?;
    }
    Ok(stats)
}

fn validate_section_name(section: &str) -> Result<(), String> {
    let section = section.trim();
    if section.is_empty() {
        return Err("behavioral_bank section names cannot be empty".into());
    }
    if section.chars().count() > defaults::MAX_BANK_SECTION_CHARS {
        return Err(format!(
            "behavioral_bank section '{section}' exceeds {} characters",
            defaults::MAX_BANK_SECTION_CHARS
        ));
    }
    if section.chars().any(|c| c.is_control()) {
        return Err(format!(
            "behavioral_bank section '{section}' contains unsupported control characters"
        ));
    }
    Ok(())
}

fn validate_questions(
    section: &str,
    questions: &[JsonValue],
    stats: &mut BankStats,
) -> Result<(), String> {
    if questions.is_empty() {
        return Err(format!(
            "behavioral_bank section '{section}' has no questions"
        ));
    }
    stats.populated_sections += 1;
    stats.total_questions += questions.len();
    for (idx, question) in questions.iter().enumerate() {
        let text = question.as_str().ok_or_else(|| {
            format!(
                "behavioral_bank section '{section}' question {} must be text",
                idx + 1
            )
        })?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "behavioral_bank section '{section}' question {} is empty",
                idx + 1
            ));
        }
        if trimmed.chars().count() > defaults::MAX_BANK_QUESTION_CHARS {
            return Err(format!(
                "behavioral_bank section '{section}' question {} exceeds {} characters",
                idx + 1,
                defaults::MAX_BANK_QUESTION_CHARS
            ));
        }
        if trimmed
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        {
            return Err(format!(
                "behavioral_bank section '{section}' question {} contains unsupported control characters",
                idx + 1
            ));
        }
    }
    Ok(())
}

/// Response body for `GET/PUT /v1/me/config`. `saved` is false only when the
/// account has no stored config and is receiving typed defaults.
#[derive(Debug, Serialize)]
pub struct HrConfigEnvelope {
    pub config: HrConfig,
    pub saved: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> HrConfig {
        HrConfig {
            hr_email: "hr@example.com".into(),
            tech_count: 5,
            behavioral_count: 1,
            behavioral_bank: json!({ "Teamwork": ["How do you handle conflict?"] }),
            pass_threshold: 70,
            voice_name: String::new(),
        }
    }

    #[test]
    fn validates_behavioral_bank_shape() {
        assert!(valid_config().validate().is_ok());

        let mut cfg = valid_config();
        cfg.behavioral_bank = json!({ "Teamwork": [] });
        assert!(cfg.validate().is_err());

        let mut cfg = valid_config();
        cfg.behavioral_count = 2;
        assert!(cfg.validate().is_err());
    }
}
