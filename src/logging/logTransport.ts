import {
  debug as writeDebugLog,
  error as writeErrorLog,
  info as writeInfoLog,
  trace as writeTraceLog,
  warn as writeWarnLog,
  type LogOptions,
} from "@tauri-apps/plugin-log";

type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

type PluginLogWriter = (message: string, options?: LogOptions) => Promise<void>;
const noop = (): void => undefined;

const pluginLogWriterByLogLevel: Record<LogLevel, PluginLogWriter> = {
  trace: writeTraceLog,
  debug: writeDebugLog,
  info: writeInfoLog,
  warn: writeWarnLog,
  error: writeErrorLog,
};

function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function escapeLogText(value: string): string {
  return value.replace(/\r/gu, "\\r").replace(/\n/gu, "\\n");
}

export function writeLog(
  level: LogLevel,
  message: string,
  keyValues: Record<string, string>,
): void {
  if (!isTauriEnvironment()) {
    return;
  }

  try {
    void pluginLogWriterByLogLevel[level](escapeLogText(message), {
      keyValues: Object.fromEntries(
        Object.entries(keyValues).map(([key, value]) => [
          key,
          escapeLogText(value),
        ]),
      ),
    }).catch(noop);
  } catch {
    noop();
  }
}
