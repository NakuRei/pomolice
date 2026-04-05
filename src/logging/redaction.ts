import {
  getFrontendLogEventDefinition,
  type FrontendEventId,
  type FrontendFieldsByEvent,
  type FrontendScope,
  type LogFieldValue,
  type RedactionClassification,
} from "./logEvents";

export const EXCEPTION_MESSAGE_FIELD = "exception.message";
export const EXCEPTION_STACKTRACE_FIELD = "exception.stacktrace";
export const EXCEPTION_TYPE_FIELD = "exception.type";
export const LOGGER_SCOPE_FIELD = "logger_scope";
export const REDACTED_VALUE = "[REDACTED]";

export interface SanitizedFrontendLogRecord {
  keyValues: Record<string, string>;
}

type ReservedFieldName =
  | typeof LOGGER_SCOPE_FIELD
  | typeof EXCEPTION_TYPE_FIELD
  | typeof EXCEPTION_MESSAGE_FIELD
  | typeof EXCEPTION_STACKTRACE_FIELD;

type ReservedLogFields = Partial<Record<ReservedFieldName, LogFieldValue>>;

const RESERVED_FIELD_CLASSIFICATION_BY_NAME = {
  [LOGGER_SCOPE_FIELD]: "public",
  [EXCEPTION_TYPE_FIELD]: "public",
  [EXCEPTION_MESSAGE_FIELD]: "sensitive",
  [EXCEPTION_STACKTRACE_FIELD]: "sensitive",
} as const satisfies Record<ReservedFieldName, RedactionClassification>;

const SENSITIVE_VALUE_MAX_LENGTH = 256;
const STACK_LIKE_SENSITIVE_VALUE_MAX_LENGTH = 2048;
const STACK_LIKE_SENSITIVE_FIELD_NAME_SET = new Set<string>([
  EXCEPTION_STACKTRACE_FIELD,
  "component_stack",
]);

let productionEnvironmentOverride: boolean | undefined = undefined;

function createHighRiskTokenPattern(): RegExp {
  return /\b(?:bearer\s+[^\s]+|(?:password|token|secret|authorization|cookie|api_key)\s*=\s*[^\s]+)/giu;
}

function isProductionEnvironment(): boolean {
  return productionEnvironmentOverride ?? import.meta.env.PROD;
}

function normalizeScope(scope: string): string {
  const trimmedScope = scope.trim();
  return trimmedScope === "" ? "unknown_scope" : trimmedScope;
}

function stringifyLogFieldValue(value: LogFieldValue): string {
  return value === null ? "null" : String(value);
}

function stringifyUnknownError(error: object): string {
  try {
    return JSON.stringify(error);
  } catch {
    return Object.prototype.toString.call(error);
  }
}

function stringifyPrimitiveError(
  error: bigint | boolean | number | string | symbol | null,
): string {
  if (error === null) {
    return "null";
  }

  return typeof error === "bigint" || typeof error === "symbol"
    ? error.toString()
    : String(error);
}

function stringifyFunctionError(error: () => unknown): string {
  return error.name === ""
    ? "<anonymous function>"
    : `[function ${error.name}]`;
}

function isCallableError(error: unknown): error is () => unknown {
  return typeof error === "function";
}

function readErrorProperty(
  error: object,
  propertyName: "message" | "name" | "stack",
): string | undefined {
  const propertyValue: unknown = Reflect.get(error, propertyName);
  if (propertyValue === undefined || propertyValue === null) {
    return undefined;
  }

  if (typeof propertyValue === "string") {
    return propertyValue;
  }

  if (
    typeof propertyValue === "bigint" ||
    typeof propertyValue === "boolean" ||
    typeof propertyValue === "number" ||
    typeof propertyValue === "symbol"
  ) {
    return propertyValue.toString();
  }

  return undefined;
}

function createExceptionFieldsFromError(error: Error): ReservedLogFields {
  return {
    [EXCEPTION_TYPE_FIELD]: error.name,
    [EXCEPTION_MESSAGE_FIELD]: error.message,
    ...(error.stack === undefined
      ? {}
      : { [EXCEPTION_STACKTRACE_FIELD]: error.stack }),
  };
}

function createExceptionFieldsFromObject(
  error: object,
): ReservedLogFields | null {
  const errorName = readErrorProperty(error, "name");
  const errorMessage = readErrorProperty(error, "message");
  const errorStack = readErrorProperty(error, "stack");

  if (
    errorName === undefined &&
    errorMessage === undefined &&
    errorStack === undefined
  ) {
    return null;
  }

  return {
    ...(errorName === undefined ? {} : { [EXCEPTION_TYPE_FIELD]: errorName }),
    ...(errorMessage === undefined
      ? {}
      : { [EXCEPTION_MESSAGE_FIELD]: errorMessage }),
    ...(errorStack === undefined
      ? {}
      : { [EXCEPTION_STACKTRACE_FIELD]: errorStack }),
  };
}

function createExceptionFieldsFromNonObject(error: unknown): ReservedLogFields {
  if (
    typeof error === "bigint" ||
    typeof error === "boolean" ||
    typeof error === "number" ||
    typeof error === "string" ||
    typeof error === "symbol" ||
    error === null
  ) {
    return {
      [EXCEPTION_MESSAGE_FIELD]: stringifyPrimitiveError(error),
    };
  }

  return {
    [EXCEPTION_MESSAGE_FIELD]: isCallableError(error)
      ? stringifyFunctionError(error)
      : "unknown error",
  };
}

function createExceptionFields(error: unknown): ReservedLogFields {
  if (error === undefined) {
    return {};
  }

  if (error instanceof Error) {
    return createExceptionFieldsFromError(error);
  }

  if (error !== null && typeof error === "object") {
    return (
      createExceptionFieldsFromObject(error) ?? {
        [EXCEPTION_MESSAGE_FIELD]: stringifyUnknownError(error),
      }
    );
  }

  return createExceptionFieldsFromNonObject(error);
}

function redactHighRiskTokens(value: string): string {
  return value.replace(createHighRiskTokenPattern(), REDACTED_VALUE);
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

function isStackLikeSensitiveFieldName(fieldName: string): boolean {
  return STACK_LIKE_SENSITIVE_FIELD_NAME_SET.has(fieldName);
}

function redactFieldValue(
  fieldName: string,
  fieldValue: LogFieldValue,
  classification: RedactionClassification,
): string {
  const stringValue = stringifyLogFieldValue(fieldValue);

  if (classification === "secret") {
    return REDACTED_VALUE;
  }

  if (classification === "sensitive") {
    return redactSensitiveValue(
      stringValue,
      isStackLikeSensitiveFieldName(fieldName)
        ? STACK_LIKE_SENSITIVE_VALUE_MAX_LENGTH
        : SENSITIVE_VALUE_MAX_LENGTH,
    );
  }

  return stringValue;
}

function assertNoUnknownFrontendFields(
  eventId: FrontendEventId,
  fieldNames: string[],
): void {
  if (fieldNames.length === 0 || isProductionEnvironment()) {
    return;
  }

  throw new Error(
    `Unknown frontend log field(s) for "${eventId}": ${fieldNames.join(", ")}`,
  );
}

function getKnownEventFieldEntries<E extends FrontendEventId>(
  eventId: E,
  fields: FrontendFieldsByEvent[E] | undefined,
): [string, string][] {
  const definition = getFrontendLogEventDefinition(eventId);
  const fieldRecord: Partial<Record<string, LogFieldValue>> = fields ?? {};
  const classificationByFieldName: Partial<
    Record<string, RedactionClassification>
  > = definition.redactionClassification;
  const allowedFieldNameSet = new Set<string>(definition.allowedFields);
  const fieldNames = Object.keys(fieldRecord);
  const unknownFieldNames = fieldNames.filter(
    (fieldName) => !allowedFieldNameSet.has(fieldName),
  );

  assertNoUnknownFrontendFields(eventId, unknownFieldNames);

  return fieldNames.flatMap((fieldName) => {
    if (!allowedFieldNameSet.has(fieldName)) {
      return [];
    }

    const fieldValue = fieldRecord[fieldName];
    if (fieldValue === undefined) {
      return [];
    }

    const classification = classificationByFieldName[fieldName];
    if (classification === undefined) {
      throw new Error(
        `Missing redaction classification for "${fieldName}" on "${eventId}"`,
      );
    }

    return [
      [fieldName, redactFieldValue(fieldName, fieldValue, classification)],
    ];
  });
}

function getReservedFieldEntries(
  scope: FrontendScope,
  error: unknown,
): [string, string][] {
  const exceptionFields = createExceptionFields(error);
  const reservedFieldEntries: [ReservedFieldName, LogFieldValue | undefined][] =
    [
      [LOGGER_SCOPE_FIELD, normalizeScope(scope)],
      [EXCEPTION_TYPE_FIELD, exceptionFields[EXCEPTION_TYPE_FIELD]],
      [EXCEPTION_MESSAGE_FIELD, exceptionFields[EXCEPTION_MESSAGE_FIELD]],
      [EXCEPTION_STACKTRACE_FIELD, exceptionFields[EXCEPTION_STACKTRACE_FIELD]],
    ];

  return reservedFieldEntries.flatMap(([fieldName, fieldValue]) => {
    if (fieldValue === undefined) {
      return [];
    }

    return [
      [
        fieldName,
        redactFieldValue(
          fieldName,
          fieldValue,
          RESERVED_FIELD_CLASSIFICATION_BY_NAME[fieldName],
        ),
      ],
    ];
  });
}

export function buildSanitizedFrontendLogRecord<E extends FrontendEventId>(
  eventId: E,
  scope: FrontendScope,
  fields: FrontendFieldsByEvent[E] | undefined,
  error: unknown,
): SanitizedFrontendLogRecord {
  return {
    keyValues: Object.fromEntries([
      ...getKnownEventFieldEntries(eventId, fields),
      ...getReservedFieldEntries(scope, error),
    ]),
  };
}

export function setProductionEnvironmentOverrideForTest(
  isProduction: boolean | undefined,
): void {
  productionEnvironmentOverride = isProduction;
}
