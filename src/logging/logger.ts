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
} from "./logEvents";
import { buildSanitizedFrontendLogRecord } from "./redaction";

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

  const { keyValues: sanitizedKeyValues } = buildSanitizedFrontendLogRecord(
    eventId,
    scope,
    fields,
    error,
  );

  if (!isTauriEnvironment()) {
    return;
  }

  const keyValues = Object.fromEntries(
    Object.entries(sanitizedKeyValues).map(([fieldName, fieldValue]) => [
      fieldName,
      escapeLogText(fieldValue),
    ]),
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
