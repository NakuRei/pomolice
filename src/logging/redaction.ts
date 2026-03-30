import {
  getFrontendLogEventDefinition,
  type FrontendEventId,
  type LogFieldValue,
  type RedactionClassification,
} from "./logEvents";

export const EXCEPTION_MESSAGE_FIELD = "exception.message";
export const EXCEPTION_STACKTRACE_FIELD = "exception.stacktrace";
export const EXCEPTION_TYPE_FIELD = "exception.type";
export const LOGGER_SCOPE_FIELD = "logger_scope";
export const REDACTED_VALUE = "[REDACTED]";

const HIGH_RISK_FIELD_NAME_PATTERN =
  /password|token|secret|authorization|cookie/iu;
const HIGH_RISK_TOKEN_PATTERN =
  /\b(?:bearer\s+[^\s]+|(?:password|token|secret|authorization|cookie)\s*[=:]\s*[^\s]+)/giu;
const SENSITIVE_VALUE_MAX_LENGTH = 256;
const STACKTRACE_MAX_LENGTH = 2048;

function stringifyLogFieldValue(value: LogFieldValue): string {
  return value === null ? "null" : String(value);
}

function redactHighRiskTokens(value: string): string {
  return value.replace(HIGH_RISK_TOKEN_PATTERN, REDACTED_VALUE);
}

function truncateSensitiveValue(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}...[TRUNCATED]`;
}

function redactSensitiveValue(value: string, maxLength: number): string {
  return truncateSensitiveValue(redactHighRiskTokens(value), maxLength);
}

function classifyField(
  eventId: FrontendEventId,
  fieldName: string,
): RedactionClassification {
  if (fieldName === LOGGER_SCOPE_FIELD || fieldName === EXCEPTION_TYPE_FIELD) {
    return "public";
  }

  if (
    fieldName === EXCEPTION_MESSAGE_FIELD ||
    fieldName === EXCEPTION_STACKTRACE_FIELD
  ) {
    return "sensitive";
  }

  const definition = getFrontendLogEventDefinition(eventId);
  const classification = definition.redactionClassification[fieldName];
  if (classification) {
    return classification;
  }

  return HIGH_RISK_FIELD_NAME_PATTERN.test(fieldName) ? "secret" : "public";
}

function redactFieldValue(
  eventId: FrontendEventId,
  fieldName: string,
  fieldValue: LogFieldValue,
): string {
  const stringValue = stringifyLogFieldValue(fieldValue);
  const classification = classifyField(eventId, fieldName);

  if (classification === "secret") {
    return REDACTED_VALUE;
  }

  if (fieldName === EXCEPTION_STACKTRACE_FIELD) {
    return redactSensitiveValue(stringValue, STACKTRACE_MAX_LENGTH);
  }

  if (classification === "sensitive") {
    return redactSensitiveValue(stringValue, SENSITIVE_VALUE_MAX_LENGTH);
  }

  return stringValue;
}

export function sanitizeFrontendLogFields(
  eventId: FrontendEventId,
  fields: Record<string, LogFieldValue>,
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(fields).map(([fieldName, fieldValue]) => [
      fieldName,
      redactFieldValue(eventId, fieldName, fieldValue),
    ]),
  );
}
