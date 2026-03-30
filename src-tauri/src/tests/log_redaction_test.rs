use crate::log_redaction::{
    redact_exception_message, redact_exception_stacktrace, redact_field_value, RedactionClass,
};

#[test]
fn test_secret_field_values_are_fully_redacted() {
    assert_eq!(
        redact_field_value(RedactionClass::Secret, "super-secret-value"),
        "[REDACTED]"
    );
}

#[test]
fn test_sensitive_field_values_are_truncated() {
    let value = "a".repeat(200);
    let redacted = redact_field_value(RedactionClass::Sensitive, &value);

    assert_eq!(redacted.len(), 160);
    assert!(redacted.ends_with("...[truncated]"));
}

#[test]
fn test_exception_message_masks_high_risk_tokens() {
    let redacted = redact_exception_message("password=abc token=xyz secret=123 api_key=456");

    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("abc"));
    assert!(!redacted.contains("xyz"));
    assert!(!redacted.contains("secret=123"));
}

#[test]
fn test_exception_stacktrace_masks_high_risk_tokens_and_truncates() {
    let stacktrace = format!("{} password=abc", "x".repeat(1100));
    let redacted = redact_exception_stacktrace(&stacktrace);

    assert!(redacted.ends_with("...[truncated]"));
    assert!(!redacted.contains("password=abc"));
}
