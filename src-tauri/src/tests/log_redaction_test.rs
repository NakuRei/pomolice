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
fn test_sensitive_field_values_mask_high_risk_assignment_tokens_case_insensitively() {
    let cases = [
        ("Password=field-password", "field-password"),
        ("TOKEN=field-token", "field-token"),
        ("Secret=field-secret", "field-secret"),
        ("AUTHORIZATION=field-authorization", "field-authorization"),
        ("Cookie=field-cookie", "field-cookie"),
        ("API_KEY=field-api-key", "field-api-key"),
    ];

    for (message, secret) in cases {
        let redacted = redact_field_value(RedactionClass::Sensitive, message);

        assert_redacted_value_hides(&redacted, secret);
        assert!(redacted.contains("[REDACTED]"));
    }
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
fn test_exception_message_masks_high_risk_assignment_tokens_case_insensitively() {
    let redacted = redact_exception_message(
        "Password=exception-password TOKEN=exception-token Secret=exception-secret AUTHORIZATION=exception-authorization Cookie=exception-cookie API_KEY=exception-api-key",
    );

    assert!(redacted.contains("[REDACTED]"));
    assert_redacted_value_hides(&redacted, "exception-password");
    assert_redacted_value_hides(&redacted, "exception-token");
    assert_redacted_value_hides(&redacted, "exception-secret");
    assert_redacted_value_hides(&redacted, "exception-authorization");
    assert_redacted_value_hides(&redacted, "exception-cookie");
    assert_redacted_value_hides(&redacted, "exception-api-key");
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
fn test_sensitive_values_mask_tokens_with_trailing_punctuation() {
    let cases = [
        (
            "request failed token=punct-token,",
            "punct-token",
            "[REDACTED],",
        ),
        (
            "request failed password=punct-password.",
            "punct-password",
            "[REDACTED].",
        ),
        (
            "request failed api_key=punct-api-key)",
            "punct-api-key",
            "[REDACTED])",
        ),
        (
            "request failed Bearer punct-bearer,",
            "punct-bearer",
            "[REDACTED],",
        ),
    ];

    for (message, secret, preserved_suffix) in cases {
        let redacted = redact_field_value(RedactionClass::Sensitive, message);

        assert_redacted_value_hides(&redacted, secret);
        assert!(redacted.contains(preserved_suffix));
    }
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
fn test_sensitive_field_value_truncation_boundary() {
    let boundary_value = "a".repeat(160);
    let over_boundary_value = "a".repeat(161);

    let boundary_redacted = redact_field_value(RedactionClass::Sensitive, &boundary_value);
    let over_boundary_redacted =
        redact_field_value(RedactionClass::Sensitive, &over_boundary_value);

    assert_eq!(boundary_redacted, boundary_value);
    assert_eq!(over_boundary_redacted.chars().count(), 160);
    assert!(over_boundary_redacted.ends_with("...[truncated]"));
}

#[test]
fn test_exception_message_truncation_boundary() {
    let boundary_message = "a".repeat(256);
    let over_boundary_message = "a".repeat(257);

    let boundary_redacted = redact_exception_message(&boundary_message);
    let over_boundary_redacted = redact_exception_message(&over_boundary_message);

    assert_eq!(boundary_redacted, boundary_message);
    assert_eq!(over_boundary_redacted.chars().count(), 256);
    assert!(over_boundary_redacted.ends_with("...[truncated]"));
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

fn assert_redacted_value_hides(redacted: &str, secret: &str) {
    assert!(
        !redacted.contains(secret),
        "redacted value should not contain {secret:?}: {redacted}"
    );
}

fn assert_home_path_is_redacted(redacted: &str, forbidden_fragments: &[&str]) {
    for fragment in forbidden_fragments {
        assert!(
            !redacted.contains(fragment),
            "redacted value should not contain {fragment:?}: {redacted}"
        );
    }
}
