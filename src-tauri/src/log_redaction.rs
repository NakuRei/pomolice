#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedactionClass {
    Public,
    Sensitive,
    Secret,
}

const REDACTED_VALUE: &str = "[REDACTED]";
const REDACTED_TRUNCATED_SUFFIX: &str = "...[truncated]";
const SENSITIVE_VALUE_LIMIT: usize = 160;
const EXCEPTION_MESSAGE_LIMIT: usize = 256;
#[cfg(test)]
const STACKTRACE_LIMIT: usize = 1024;

pub(crate) fn redact_field_value(classification: RedactionClass, value: &str) -> String {
    match classification {
        RedactionClass::Public => escape_log_component(value),
        RedactionClass::Sensitive => escape_log_component(&truncate_redacted_value(
            &mask_high_risk_tokens(value),
            SENSITIVE_VALUE_LIMIT,
        )),
        RedactionClass::Secret => REDACTED_VALUE.to_string(),
    }
}

pub(crate) fn redact_exception_message(value: &str) -> String {
    escape_log_component(&truncate_redacted_value(
        &mask_high_risk_tokens(value),
        EXCEPTION_MESSAGE_LIMIT,
    ))
}

#[cfg(test)]
pub(crate) fn redact_exception_stacktrace(value: &str) -> String {
    escape_log_component(&truncate_redacted_value(
        &mask_high_risk_tokens(value),
        STACKTRACE_LIMIT,
    ))
}

pub(crate) fn escape_log_component(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('=', "\\=")
        .replace('"', "\\\"")
        .replace(' ', "\\ ")
}

fn truncate_redacted_value(value: &str, limit: usize) -> String {
    let character_count = value.chars().count();
    if character_count <= limit {
        return value.to_string();
    }

    let visible_limit = limit.saturating_sub(REDACTED_TRUNCATED_SUFFIX.len());
    let mut truncated = value.chars().take(visible_limit).collect::<String>();
    truncated.push_str(REDACTED_TRUNCATED_SUFFIX);
    truncated
}

fn mask_high_risk_tokens(value: &str) -> String {
    let mut masked_segments = Vec::new();

    for segment in value.split_whitespace() {
        masked_segments.push(mask_high_risk_segment(segment));
    }

    masked_segments.join(" ")
}

fn mask_high_risk_segment(segment: &str) -> String {
    let stripped = segment.trim_end_matches(|character: char| {
        matches!(character, ',' | '.' | ';' | ':' | ')' | ']' | '}')
    });
    let trailing = &segment[stripped.len()..];

    if let Some(redacted) = redact_assignment_token(stripped, "password") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_assignment_token(stripped, "token") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_assignment_token(stripped, "secret") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_assignment_token(stripped, "cookie") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_assignment_token(stripped, "authorization") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_assignment_token(stripped, "api_key") {
        return format!("{redacted}{trailing}");
    }

    if let Some(redacted) = redact_bearer_token(stripped) {
        return format!("{redacted}{trailing}");
    }

    segment.to_string()
}

fn redact_assignment_token(segment: &str, key: &str) -> Option<String> {
    let (raw_key, _raw_value) = segment.split_once('=')?;
    if !raw_key.eq_ignore_ascii_case(key) {
        return None;
    }

    Some(format!("{raw_key}={REDACTED_VALUE}"))
}

fn redact_bearer_token(segment: &str) -> Option<String> {
    let prefix = "bearer";
    if segment.len() < prefix.len() {
        return None;
    }

    if !segment[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }

    let remainder = segment[prefix.len()..].trim_start();
    if remainder.is_empty() {
        return Some(prefix.to_string());
    }

    Some(format!("{prefix} {REDACTED_VALUE}"))
}
