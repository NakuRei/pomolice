import { beforeEach, describe, expect, it, vi } from "vitest";

const traceLog = vi.fn().mockResolvedValue(undefined);
const debugLog = vi.fn().mockResolvedValue(undefined);
const infoLog = vi.fn().mockResolvedValue(undefined);
const warnLog = vi.fn().mockResolvedValue(undefined);
const errorLog = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/plugin-log", () => ({
  trace: traceLog,
  debug: debugLog,
  info: infoLog,
  warn: warnLog,
  error: errorLog,
}));

function getTauriInternals(): unknown {
  return Reflect.get(globalThis, "__TAURI_INTERNALS__");
}

function setTauriInternals(value: unknown): void {
  Reflect.set(globalThis, "__TAURI_INTERNALS__", value);
}

describe("writeLog()", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it("maps frontend log levels to plugin log levels", async () => {
    const { writeLog } = await import("./logTransport");

    writeLog("trace", "timer_sync.trace_event", { sample: "trace" });
    writeLog("debug", "timer_sync.debug_event", { sample: "debug" });
    writeLog("info", "timer_sync.info_event", { sample: "info" });
    writeLog("warn", "timer_sync.warn_event", { sample: "warn" });
    writeLog("error", "timer_sync.error_event", { sample: "error" });

    expect(traceLog).toHaveBeenCalledWith("timer_sync.trace_event", {
      keyValues: { sample: "trace" },
    });
    expect(debugLog).toHaveBeenCalledWith("timer_sync.debug_event", {
      keyValues: { sample: "debug" },
    });
    expect(infoLog).toHaveBeenCalledWith("timer_sync.info_event", {
      keyValues: { sample: "info" },
    });
    expect(warnLog).toHaveBeenCalledWith("timer_sync.warn_event", {
      keyValues: { sample: "warn" },
    });
    expect(errorLog).toHaveBeenCalledWith("timer_sync.error_event", {
      keyValues: { sample: "error" },
    });
  });

  it("passes stringified key values through without reinterpreting them", async () => {
    const { writeLog } = await import("./logTransport");

    writeLog("info", "notifications.preview_failed", {
      volume_percent: "60",
      muted: "false",
    });

    expect(infoLog).toHaveBeenCalledWith("notifications.preview_failed", {
      keyValues: {
        volume_percent: "60",
        muted: "false",
      },
    });
  });

  it("escapes CRLF characters in the message and key values", async () => {
    const { writeLog } = await import("./logTransport");

    writeLog("warn", "notifications.preview_failed\nforged_entry", {
      failure_message: "line1\r\nline2",
    });

    expect(warnLog).toHaveBeenCalledWith(
      "notifications.preview_failed\\nforged_entry",
      {
        keyValues: {
          failure_message: "line1\\r\\nline2",
        },
      },
    );
  });

  it("does not call plugin log functions outside Tauri", async () => {
    const originalTauriInternals = getTauriInternals();
    Reflect.deleteProperty(globalThis, "__TAURI_INTERNALS__");

    try {
      const { writeLog } = await import("./logTransport");

      expect(() => {
        writeLog("info", "bootstrap.completed", {});
      }).not.toThrow();

      expect(traceLog).not.toHaveBeenCalled();
      expect(debugLog).not.toHaveBeenCalled();
      expect(infoLog).not.toHaveBeenCalled();
      expect(warnLog).not.toHaveBeenCalled();
      expect(errorLog).not.toHaveBeenCalled();
    } finally {
      setTauriInternals(originalTauriInternals);
    }
  });

  it("swallows plugin write rejections", async () => {
    errorLog.mockRejectedValueOnce(new Error("plugin down"));
    const { writeLog } = await import("./logTransport");

    expect(() => {
      writeLog("error", "timer_sync.error_event", {});
    }).not.toThrow();

    await Promise.resolve();
    expect(errorLog).toHaveBeenCalledWith("timer_sync.error_event", {
      keyValues: {},
    });
  });
});
