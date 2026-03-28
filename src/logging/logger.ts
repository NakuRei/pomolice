import { writeLog } from "./logTransport";

export type LogFieldValue = string | number | boolean | null;
export type LogFields = Record<string, LogFieldValue>;

export interface FrontendLogger {
  trace(event: string, fields?: LogFields): void;
  debug(event: string, fields?: LogFields): void;
  info(event: string, fields?: LogFields): void;
  warn(event: string, fields?: LogFields): void;
  error(event: string, error?: unknown, fields?: LogFields): void;
}

const REDACTED_VALUE = "[REDACTED]";
const redactedFieldPattern = /password|token|secret|authorization|cookie/iu;
const noop = (): void => undefined;
const EXCEPTION_TYPE_FIELD = "exception.type";
const EXCEPTION_MESSAGE_FIELD = "exception.message";
const EXCEPTION_STACKTRACE_FIELD = "exception.stacktrace";

type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

// This init gate is intentionally shared across all logger instances.
// Tests reset the module to isolate this state.
let loggingInitialized = false;

function normalizeIdentifier(value: string, fallback: string): string {
  const trimmedValue = value.trim();
  return trimmedValue === "" ? fallback : trimmedValue;
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

  return typeof error === "symbol" || typeof error === "bigint"
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
  propertyName: "name" | "message" | "stack",
): string | undefined {
  const propertyValue: unknown = Reflect.get(error, propertyName);
  if (propertyValue === undefined || propertyValue === null) {
    return undefined;
  }

  if (typeof propertyValue === "string") {
    return propertyValue;
  }

  if (
    typeof propertyValue === "number" ||
    typeof propertyValue === "boolean" ||
    typeof propertyValue === "bigint" ||
    typeof propertyValue === "symbol"
  ) {
    return propertyValue.toString();
  }

  return undefined;
}

function createExceptionFieldsFromError(error: Error): LogFields {
  return {
    [EXCEPTION_TYPE_FIELD]: error.name,
    [EXCEPTION_MESSAGE_FIELD]: error.message,
    ...(error.stack !== undefined
      ? { [EXCEPTION_STACKTRACE_FIELD]: error.stack }
      : {}),
  };
}

function createExceptionFieldsFromObject(error: object): LogFields | null {
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
    ...(errorName !== undefined ? { [EXCEPTION_TYPE_FIELD]: errorName } : {}),
    ...(errorMessage !== undefined
      ? { [EXCEPTION_MESSAGE_FIELD]: errorMessage }
      : {}),
    ...(errorStack !== undefined
      ? { [EXCEPTION_STACKTRACE_FIELD]: errorStack }
      : {}),
  };
}

function createExceptionFieldsFromNonObject(error: unknown): LogFields {
  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean" ||
    typeof error === "bigint" ||
    typeof error === "symbol" ||
    error === null
  ) {
    return { [EXCEPTION_MESSAGE_FIELD]: stringifyPrimitiveError(error) };
  }

  return {
    [EXCEPTION_MESSAGE_FIELD]: isCallableError(error)
      ? stringifyFunctionError(error)
      : "unknown error",
  };
}

function createExceptionFields(error: unknown): LogFields {
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

function sanitizeFields(fields?: LogFields): Record<string, string> {
  if (!fields) {
    return {};
  }

  return Object.fromEntries(
    Object.entries(fields).map(([key, value]) => [
      key,
      redactedFieldPattern.test(key)
        ? REDACTED_VALUE
        : stringifyLogFieldValue(value),
    ]),
  );
}

function writeRecord(
  level: LogLevel,
  scope: string,
  event: string,
  error?: unknown,
  fields?: LogFields,
): void {
  if (!loggingInitialized) {
    return;
  }

  const normalizedEvent = normalizeIdentifier(event, "unknown_event");
  const mergedFields: LogFields = {
    ...(fields ?? {}),
    ...createExceptionFields(error),
  };

  try {
    writeLog(
      level,
      `${scope}.${normalizedEvent}`,
      sanitizeFields(mergedFields),
    );
  } catch {
    noop();
  }
}

function log(
  level: LogLevel,
  scope: string,
  event: string,
  fields?: LogFields,
): void {
  writeRecord(level, scope, event, undefined, fields);
}

function logError(
  scope: string,
  event: string,
  error?: unknown,
  fields?: LogFields,
): void {
  writeRecord("error", scope, event, error, fields);
}

export function createLogger(scope: string): FrontendLogger {
  const normalizedScope = normalizeIdentifier(scope, "unknown_scope");

  return {
    trace: (event, fields): void => {
      log("trace", normalizedScope, event, fields);
    },
    debug: (event, fields): void => {
      log("debug", normalizedScope, event, fields);
    },
    info: (event, fields): void => {
      log("info", normalizedScope, event, fields);
    },
    warn: (event, fields): void => {
      log("warn", normalizedScope, event, fields);
    },
    error: (event, error, fields): void => {
      logError(normalizedScope, event, error, fields);
    },
  };
}

export function initializeFrontendLogging(): Promise<void> {
  loggingInitialized = true;
  return Promise.resolve();
}
