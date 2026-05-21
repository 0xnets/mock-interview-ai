pub const MAX_CANDIDATE_NAME_CHARS: usize = 120;
pub const MAX_ROLE_TITLE_CHARS: usize = 160;
pub const MIN_JD_WORDS: usize = 20;
pub const MIN_RESUME_WORDS: usize = 30;

pub fn validate_email(value: &str, field: &str) -> Result<(), String> {
    let email = value.trim();
    if email.is_empty() {
        return Err(format!("{field} is required"));
    }
    if email.len() > 254 || email.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(format!("{field} looks invalid"));
    }

    let Some((local, domain)) = email.split_once('@') else {
        return Err(format!("{field} looks invalid"));
    };
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || domain.len() > 253
        || domain.contains('@')
    {
        return Err(format!("{field} looks invalid"));
    }
    if !local
        .bytes()
        .all(|b| b.is_ascii_graphic() && !matches!(b, b'@' | b',' | b';' | b':' | b'<' | b'>'))
    {
        return Err(format!("{field} looks invalid"));
    }

    let mut labels = domain.split('.');
    let mut label_count = 0usize;
    for label in labels.by_ref() {
        label_count += 1;
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(format!("{field} looks invalid"));
        }
    }
    if label_count < 2 {
        return Err(format!("{field} looks invalid"));
    }
    Ok(())
}

pub fn validate_short_text(value: &str, field: &str, max_chars: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed.chars().count() > max_chars {
        return Err(format!("{field} must be {max_chars} characters or fewer"));
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r')
    {
        return Err(format!("{field} contains unsupported control characters"));
    }
    Ok(())
}

pub fn validate_document_text(
    value: &str,
    field: &str,
    max_bytes: usize,
    min_words: usize,
) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} exceeds {max_bytes} bytes"));
    }
    if trimmed.chars().any(|c| c == '\u{fffd}') {
        return Err(format!(
            "{field} contains unreadable replacement characters; paste cleaner text"
        ));
    }
    if trimmed
        .chars()
        .filter(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        .count()
        > 0
    {
        return Err(format!("{field} contains unsupported control characters"));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("pdf extraction failed") || lower.contains("could not extract") {
        return Err(format!(
            "{field} appears to contain a PDF extraction error instead of document text"
        ));
    }
    let word_count = trimmed
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphabetic()))
        .count();
    if word_count < min_words {
        return Err(format!("{field} must contain at least {min_words} words"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_common_email_shapes() {
        assert!(validate_email("admin@local.test", "email").is_ok());
        assert!(validate_email("bad", "email").is_err());
        assert!(validate_email("a@localhost", "email").is_err());
        assert!(validate_email("a@-example.com", "email").is_err());
    }

    #[test]
    fn rejects_too_short_document_text() {
        assert!(validate_document_text("too short", "resume_text", 1024, 3).is_err());
        assert!(validate_document_text("one two three", "resume_text", 1024, 3).is_ok());
    }
}
