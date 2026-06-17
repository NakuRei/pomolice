use crate::log_redaction::{redact_exception_message, redact_field_value, RedactionClass};

#[test]
fn test_secret_field_values_are_fully_redacted() {
    assert_eq!(
        redact_field_value(RedactionClass::Secret, "super-secret-value"),
        "[REDACTED]"
    );
}

#[test]
fn test_sensitive_field_values_are_truncated() {
    let value = "a".repeat(10_000);
    let redacted = redact_field_value(RedactionClass::Sensitive, &value);

    assert!(redacted.chars().count() < value.chars().count());
    assert_ne!(redacted, value);
    assert!(redacted.ends_with("...[truncated]"));
}

#[test]
fn test_sensitive_field_values_mask_high_risk_tokens_when_truncated() {
    let value = format!("api_key=field-secret {}", "a".repeat(10_000));
    let redacted = redact_field_value(RedactionClass::Sensitive, &value);

    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("field-secret"));
    assert!(redacted.ends_with("...[truncated]"));
}

#[test]
fn test_api_key_is_masked_by_high_risk_token_rule() {
    let redacted = redact_field_value(RedactionClass::Sensitive, "api_key=abc123 bearer token=xyz");

    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("abc123"));
    assert!(!redacted.contains("token=xyz"));
}

#[test]
fn test_sensitive_field_values_mask_authorization_bearer_tokens() {
    let redacted = redact_field_value(
        RedactionClass::Sensitive,
        "request failed: Authorization: Bearer abc123",
    );

    assert!(redacted.contains("[REDACTED]"));
    assert!(redacted.contains("request"));
    assert!(redacted.contains("failed"));
    assert!(!redacted.contains("abc123"));
}

#[test]
fn test_exception_message_masks_same_token_vocabulary_as_frontend() {
    let redacted = redact_exception_message(
        "password=abc token=xyz secret=123 authorization=Bearer cookie=session api_key=456",
    );

    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("abc"));
    assert!(!redacted.contains("xyz"));
    assert!(!redacted.contains("123"));
    assert!(!redacted.contains("session"));
    assert!(!redacted.contains("456"));
}

#[test]
fn test_exception_message_masks_authorization_bearer_tokens() {
    let redacted = redact_exception_message("request failed: Authorization: Bearer abc123");

    assert!(redacted.contains("[REDACTED]"));
    assert!(redacted.contains("request"));
    assert!(redacted.contains("failed"));
    assert!(!redacted.contains("abc123"));
}

#[test]
fn test_exception_messages_are_truncated() {
    let message = "a".repeat(10_000);
    let redacted = redact_exception_message(&message);

    assert!(redacted.chars().count() < message.chars().count());
    assert_ne!(redacted, message);
    assert!(redacted.ends_with("...[truncated]"));
}

#[test]
fn test_public_field_value_redacts_windows_profile_path() {
    let redacted = redact_field_value(
        RedactionClass::Public,
        r"C:\Users\Alice\AppData\Local\pomolice\timer_state.json",
    );

    assert_home_path_is_redacted(&redacted, &["Alice", r"C:\Users\Alice"]);
}

#[test]
fn test_sensitive_field_value_redacts_windows_profile_path() {
    let redacted = redact_field_value(
        RedactionClass::Sensitive,
        r"C:\Users\Alice\AppData\Local\pomolice\timer_state.json",
    );

    assert_home_path_is_redacted(&redacted, &["Alice", r"C:\Users\Alice"]);
}

#[test]
fn test_exception_message_redacts_windows_profile_path() {
    let redacted = redact_exception_message(
        r"failed to open C:\Users\Alice\AppData\Local\pomolice\timer_state.json",
    );

    assert_home_path_is_redacted(&redacted, &["Alice", r"C:\Users\Alice"]);
}

#[test]
fn test_field_values_redact_unix_home_paths() {
    let macos_path = "/Users/alice/Library/Application Support/pomolice/timer_state.json";
    let linux_path = "/home/alice/.config/pomolice/timer_state.json";

    let macos_redacted = redact_field_value(RedactionClass::Public, macos_path);
    let linux_redacted = redact_field_value(RedactionClass::Sensitive, linux_path);

    assert_home_path_is_redacted(&macos_redacted, &["alice", "/Users/alice"]);
    assert_home_path_is_redacted(&linux_redacted, &["alice", "/home/alice"]);
}

#[test]
fn test_field_values_redact_profile_paths_with_spaces() {
    let path_cases = [
        (
            r"C:\Users\Jane Doe\AppData\Local\pomolice\timer_state.json",
            [r"C:\Users\Jane Doe", "Jane Doe", "Jane", "Doe"],
        ),
        (
            "/Users/Jane Doe/Library/Application Support/pomolice/timer_state.json",
            ["/Users/Jane Doe", "Jane Doe", "Jane", "Doe"],
        ),
        (
            "/home/Jane Doe/.config/pomolice/timer_state.json",
            ["/home/Jane Doe", "Jane Doe", "Jane", "Doe"],
        ),
    ];

    for (path, forbidden_fragments) in path_cases {
        let public_redacted = redact_field_value(RedactionClass::Public, path);
        let sensitive_redacted = redact_field_value(RedactionClass::Sensitive, path);
        let exception_redacted = redact_exception_message(&format!("failed to open {path}"));

        assert_home_path_is_redacted(&public_redacted, &forbidden_fragments);
        assert_home_path_is_redacted(&sensitive_redacted, &forbidden_fragments);
        assert_home_path_is_redacted(&exception_redacted, &forbidden_fragments);
    }
}

#[test]
fn test_field_values_redact_full_profile_paths_when_suffix_repeats_username() {
    let path_cases = [
        (
            r"C:\Users\Alice\Documents\Alice-notes.txt",
            ["Alice", r"C:\Users\Alice"],
        ),
        (
            "/Users/alice/Documents/alice-notes.txt",
            ["alice", "/Users/alice"],
        ),
        ("/home/alice/docs/alice-notes.txt", ["alice", "/home/alice"]),
    ];

    for (path, forbidden_fragments) in path_cases {
        let public_redacted = redact_field_value(RedactionClass::Public, path);
        let sensitive_redacted = redact_field_value(RedactionClass::Sensitive, path);
        let exception_redacted = redact_exception_message(&format!("failed to open {path}"));

        assert_home_path_is_redacted(&public_redacted, &forbidden_fragments);
        assert_home_path_is_redacted(&sensitive_redacted, &forbidden_fragments);
        assert_home_path_is_redacted(&exception_redacted, &forbidden_fragments);
    }
}

fn assert_home_path_is_redacted(redacted: &str, forbidden_fragments: &[&str]) {
    for fragment in forbidden_fragments {
        assert!(
            !redacted.contains(fragment),
            "redacted value should not contain {fragment:?}: {redacted}"
        );
    }
}
