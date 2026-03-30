import {
  debug as writeDebugLog,
  error as writeErrorLog,
  info as writeInfoLog,
  trace as writeTraceLog,
  warn as writeWarnLog,
  type LogOptions,
} from "@tauri-apps/plugin-log";
import {
  getFrontendLogEventDefinition,
  isFrontendEventId,
  type FrontendEventId,
  type FrontendFieldsByEvent,
  type FrontendLogSeverity,
  type FrontendScope,
  type LogFieldValue,
} from "./logEvents";
import {
  EXCEPTION_MESSAGE_FIELD,
  EXCEPTION_STACKTRACE_FIELD,
  EXCEPTION_TYPE_FIELD,
  LOGGER_SCOPE_FIELD,
  sanitizeFrontendLogFields,
} from "./redaction";

export type {
  FrontendEventId,
  FrontendScope,
  LogFieldValue,
} from "./logEvents";

export interface FrontendLogger {
  trace<E extends FrontendEventId>(
    eventId: E,
    fields?: FrontendFieldsByEvent[E],
  ): void;
  debug<E extends FrontendEventId>(
    eventId: E,
    fields?: FrontendFieldsByEvent[E],
  ): void;
  info<E extends FrontendEventId>(
    eventId: E,
    fields?: FrontendFieldsByEvent[E],
  ): void;
  warn<E extends FrontendEventId>(
    eventId: E,
    fields?: FrontendFieldsByEvent[E],
  ): void;
  error<E extends FrontendEventId>(
    eventId: E,
    error?: unknown,
    fields?: FrontendFieldsByEvent[E],
  ): void;
}

type LogFields = Record<string, LogFieldValue>;
type PluginLogWriter = (message: string, options?: LogOptions) => Promise<void>;

const pluginLogWriterByLogLevel: Record<FrontendLogSeverity, PluginLogWriter> =
  {
    trace: writeTraceLog,
    debug: writeDebugLog,
    info: writeInfoLog,
    warn: writeWarnLog,
    error: writeErrorLog,
  };

const noop = (): void => undefined;

function normalizeScope(scope: string): string {
  const trimmedScope = scope.trim();
  return trimmedScope === "" ? "unknown_scope" : trimmedScope;
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

function createExceptionFieldsFromError(error: Error): LogFields {
  return {
    [EXCEPTION_TYPE_FIELD]: error.name,
    [EXCEPTION_MESSAGE_FIELD]: error.message,
    ...(error.stack === undefined
      ? {}
      : { [EXCEPTION_STACKTRACE_FIELD]: error.stack }),
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
    ...(errorName === undefined ? {} : { [EXCEPTION_TYPE_FIELD]: errorName }),
    ...(errorMessage === undefined
      ? {}
      : { [EXCEPTION_MESSAGE_FIELD]: errorMessage }),
    ...(errorStack === undefined
      ? {}
      : { [EXCEPTION_STACKTRACE_FIELD]: errorStack }),
  };
}

function createExceptionFieldsFromNonObject(error: unknown): LogFields {
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

function assertKnownFrontendEventId(
  value: string,
): asserts value is FrontendEventId {
  if (!isFrontendEventId(value)) {
    throw new Error(`Unknown frontend log event: ${value}`);
  }
}

function assertEventSeverity(
  eventId: FrontendEventId,
  level: FrontendLogSeverity,
): void {
  const definition = getFrontendLogEventDefinition(eventId);
  if (definition.severity !== level) {
    throw new Error(
      `Frontend log event "${eventId}" must be logged at ${definition.severity}`,
    );
  }
}

function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function escapeLogText(value: string): string {
  return value.replace(/\r/gu, "\\r").replace(/\n/gu, "\\n");
}

function writeRecord<E extends FrontendEventId>(
  level: FrontendLogSeverity,
  scope: FrontendScope,
  eventId: E,
  error?: unknown,
  fields?: FrontendFieldsByEvent[E],
): void {
  assertKnownFrontendEventId(eventId);
  assertEventSeverity(eventId, level);

  if (!isTauriEnvironment()) {
    return;
  }

  const mergedFields: LogFields = {
    [LOGGER_SCOPE_FIELD]: normalizeScope(scope),
    ...(fields ?? {}),
    ...createExceptionFields(error),
  };

  const keyValues = Object.fromEntries(
    Object.entries(sanitizeFrontendLogFields(eventId, mergedFields)).map(
      ([fieldName, fieldValue]) => [fieldName, escapeLogText(fieldValue)],
    ),
  );

  try {
    void pluginLogWriterByLogLevel[level](escapeLogText(eventId), {
      keyValues,
    }).catch(noop);
  } catch {
    noop();
  }
}

export function createLogger(scope: FrontendScope): FrontendLogger {
  return {
    trace: (eventId, fields): void => {
      writeRecord("trace", scope, eventId, undefined, fields);
    },
    debug: (eventId, fields): void => {
      writeRecord("debug", scope, eventId, undefined, fields);
    },
    info: (eventId, fields): void => {
      writeRecord("info", scope, eventId, undefined, fields);
    },
    warn: (eventId, fields): void => {
      writeRecord("warn", scope, eventId, undefined, fields);
    },
    error: (eventId, error, fields): void => {
      writeRecord("error", scope, eventId, error, fields);
    },
  };
}
