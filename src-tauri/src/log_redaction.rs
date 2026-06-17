#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedactionClass {
    Public,
    Sensitive,
    Secret,
}

const REDACTED_VALUE: &str = "[REDACTED]";
const REDACTED_HOME_PROFILE_PATH: &str = "[USER_HOME]";
const REDACTED_TRUNCATED_SUFFIX: &str = "...[truncated]";
const SENSITIVE_VALUE_LIMIT: usize = 160;
const EXCEPTION_MESSAGE_LIMIT: usize = 256;
const HIGH_RISK_ASSIGNMENT_KEYS: [&str; 6] = [
    "password",
    "token",
    "secret",
    "authorization",
    "cookie",
    "api_key",
];
const HIGH_RISK_BEARER_PREFIX: &str = "bearer";

pub(crate) fn redact_field_value(classification: RedactionClass, value: &str) -> String {
    match classification {
        RedactionClass::Public => escape_log_component(&redact_home_profile_paths(value)),
        RedactionClass::Sensitive => escape_log_component(&truncate_redacted_value(
            &mask_high_risk_tokens(&redact_home_profile_paths(value)),
            SENSITIVE_VALUE_LIMIT,
        )),
        RedactionClass::Secret => REDACTED_VALUE.to_string(),
    }
}

pub(crate) fn redact_exception_message(value: &str) -> String {
    escape_log_component(&truncate_redacted_value(
        &mask_high_risk_tokens(&redact_home_profile_paths(value)),
        EXCEPTION_MESSAGE_LIMIT,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HomeProfilePathMatch {
    end: usize,
    start: usize,
}

fn redact_home_profile_paths(value: &str) -> String {
    let mut redacted = String::new();
    let mut cursor = 0;
    let mut found_path = false;

    while let Some(home_path) = find_next_home_profile_path(value, cursor) {
        redacted.push_str(&value[cursor..home_path.start]);
        redacted.push_str(REDACTED_HOME_PROFILE_PATH);
        cursor = home_path.end;
        found_path = true;
    }

    if !found_path {
        return value.to_string();
    }

    redacted.push_str(&value[cursor..]);
    redacted
}

fn find_next_home_profile_path(value: &str, start: usize) -> Option<HomeProfilePathMatch> {
    for (offset, _) in value[start..].char_indices() {
        let index = start + offset;
        if let Some(home_path) = find_windows_profile_path_at(value, index) {
            return Some(home_path);
        }

        for prefix in ["/Users/", "/home/"] {
            if let Some(home_path) = find_unix_home_path_at(value, index, prefix) {
                return Some(home_path);
            }
        }
    }

    None
}

fn find_windows_profile_path_at(value: &str, index: usize) -> Option<HomeProfilePathMatch> {
    let bytes = value.as_bytes();
    if !bytes.get(index).is_some_and(u8::is_ascii_alphabetic)
        || bytes.get(index + 1) != Some(&b':')
        || !bytes
            .get(index + 2)
            .is_some_and(|byte| is_path_separator(*byte))
    {
        return None;
    }

    let users_start = index + 3;
    let users_end = users_start + "Users".len();
    if !value
        .get(users_start..users_end)
        .is_some_and(|segment| segment.eq_ignore_ascii_case("Users"))
        || !bytes
            .get(users_end)
            .is_some_and(|byte| is_path_separator(*byte))
    {
        return None;
    }

    let username_start = users_end + 1;
    find_home_profile_username_end(value, username_start).map(|username_end| HomeProfilePathMatch {
        end: find_home_profile_path_end(value, username_end),
        start: index,
    })
}

fn find_unix_home_path_at(value: &str, index: usize, prefix: &str) -> Option<HomeProfilePathMatch> {
    if !value
        .get(index..)
        .is_some_and(|text| text.starts_with(prefix))
    {
        return None;
    }

    let username_start = index + prefix.len();
    find_home_profile_username_end(value, username_start).map(|username_end| HomeProfilePathMatch {
        end: find_home_profile_path_end(value, username_end),
        start: index,
    })
}

fn find_home_profile_username_end(value: &str, username_start: usize) -> Option<usize> {
    if username_start >= value.len() {
        return None;
    }

    for (offset, character) in value[username_start..].char_indices() {
        if is_home_profile_username_delimiter(character) {
            if offset == 0 {
                return None;
            }

            return Some(username_start + offset);
        }
    }

    Some(value.len())
}

fn find_home_profile_path_end(value: &str, username_end: usize) -> usize {
    let mut path_end = username_end;

    while value
        .as_bytes()
        .get(path_end)
        .is_some_and(|byte| is_path_separator(*byte))
    {
        let component_start = path_end + 1;
        let component_end = find_home_profile_path_component_end(value, component_start);
        if component_end == component_start {
            break;
        }

        path_end = component_end;
    }

    path_end
}

fn find_home_profile_path_component_end(value: &str, component_start: usize) -> usize {
    if component_start >= value.len() {
        return component_start;
    }

    for (offset, character) in value[component_start..].char_indices() {
        if character == '\\' || character == '/' || is_home_profile_path_text_delimiter(character) {
            return component_start + offset;
        }
    }

    value.len()
}

fn is_path_separator(byte: u8) -> bool {
    matches!(byte, b'\\' | b'/')
}

fn is_home_profile_username_delimiter(character: char) -> bool {
    character == '\\' || character == '/' || is_home_profile_path_text_delimiter(character)
}

fn is_home_profile_path_text_delimiter(character: char) -> bool {
    matches!(character, '\r' | '\n' | '\t')
        || matches!(character, '"' | '\'' | ',' | ';' | ':' | ')' | ']' | '}')
}

fn mask_high_risk_tokens(value: &str) -> String {
    let segments = value.split_whitespace().collect::<Vec<_>>();
    let mut masked_segments = Vec::new();
    let mut index = 0;

    while index < segments.len() {
        if let Some((redacted, consumed_segments)) =
            redact_bearer_scheme_token(segments[index], segments.get(index + 1).copied())
        {
            masked_segments.push(redacted);
            index += consumed_segments;
            continue;
        }

        let segment = segments[index];
        masked_segments.push(mask_high_risk_segment(segment));
        index += 1;
    }

    masked_segments.join(" ")
}

fn mask_high_risk_segment(segment: &str) -> String {
    let (stripped, trailing) = split_trailing_log_punctuation(segment);

    for key in HIGH_RISK_ASSIGNMENT_KEYS {
        if let Some(redacted) = redact_assignment_token(stripped, key) {
            return format!("{redacted}{trailing}");
        }
    }

    if let Some(redacted) = redact_bearer_token(stripped) {
        return format!("{redacted}{trailing}");
    }

    segment.to_string()
}

fn split_trailing_log_punctuation(segment: &str) -> (&str, &str) {
    let stripped = segment.trim_end_matches(|character: char| {
        matches!(character, ',' | '.' | ';' | ':' | ')' | ']' | '}')
    });
    let trailing = &segment[stripped.len()..];

    (stripped, trailing)
}

fn redact_assignment_token(segment: &str, key: &str) -> Option<String> {
    let (raw_key, _raw_value) = segment.split_once('=')?;
    if !raw_key.eq_ignore_ascii_case(key) {
        return None;
    }

    Some(format!("{raw_key}={REDACTED_VALUE}"))
}

fn redact_bearer_scheme_token(
    segment: &str,
    next_segment: Option<&str>,
) -> Option<(String, usize)> {
    let (scheme, scheme_trailing) = split_trailing_log_punctuation(segment);
    if !scheme.eq_ignore_ascii_case(HIGH_RISK_BEARER_PREFIX) {
        return None;
    }

    let next_segment = next_segment?;
    let (token, token_trailing) = split_trailing_log_punctuation(next_segment);
    if token.is_empty() {
        return None;
    }

    Some((
        format!("{scheme}{scheme_trailing} {REDACTED_VALUE}{token_trailing}"),
        2,
    ))
}

fn redact_bearer_token(segment: &str) -> Option<String> {
    let prefix = HIGH_RISK_BEARER_PREFIX;
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
