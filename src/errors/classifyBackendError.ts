import { ErrorSeverity } from "../store/errorStore";

/**
 * The result of classifying a raw Rust backend error string.
 *
 * Example:
 * ```ts
 * classifyBackendError("Timer already running", "start_work")
 * // → { userMessage: "すでに実行中です", isTransient: false, severity: "recoverable" }
 *
 * classifyBackendError("Rate limited: please wait 300ms", "start_work")
 * // → { userMessage: "少々お待ちください", isTransient: true, severity: "recoverable" }
 * ```
 */
export interface BackendErrorClassification {
  /** Japanese message to display in the UI. */
  userMessage: string;
  /**
   * Whether the error is likely to resolve on its own (e.g. a lock contention or rate limit).
   * Transient errors may be retried automatically by the caller.
   * Non-transient errors require user action or state change to resolve.
   */
  isTransient: boolean;
  severity: ErrorSeverity;
}

/**
 * The Tauri command that produced the error.
 * Used to adjust severity — e.g. the same "Failed to lock state" error
 * is "critical" for emergency_stop but only "recoverable" for start_work.
 */
export type CommandContext =
  | "start_work"
  | "resolve_post_break_decision"
  | "emergency_stop"
  | "get_full_state"
  | "resolve_boot_auto_start_prompt";

/**
 * Parse the error string serialized by PomoliceError::serialize() in the Rust backend
 * and map it to a user-visible classification.
 *
 * The Rust backend serializes errors as plain strings via `self.to_string()`.
 * Known patterns (from src-tauri/src/error.rs):
 *   "Timer already running"
 *   "Timer not running"
 *   "Invalid state transition: cannot {action} from {state}"
 *   "Rate limited: please wait {N}ms"
 *   "Failed to lock state: {detail}"
 *   "Window control error: {detail}"
 *   "Persistence error: {detail}"
 *   "Input monitoring error: {detail}"
 *   "Unauthorized window: {detail}"
 */
export function classifyBackendError(
  errorString: string,
  context: CommandContext
): BackendErrorClassification {
  // "Timer already running" — state mismatch, will not resolve on its own
  if (errorString === "Timer already running") {
    return {
      userMessage: "すでに実行中です",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Timer not running" — state mismatch, will not resolve on its own
  if (errorString === "Timer not running") {
    return {
      userMessage: "タイマーが停止しています",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Invalid state transition: cannot {action} from {state}"
  if (errorString.startsWith("Invalid state transition:")) {
    return {
      userMessage: "現在の状態では実行できません",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Rate limited: please wait {N}ms" — transient, will resolve after the wait
  if (errorString.startsWith("Rate limited:")) {
    return {
      userMessage: "少々お待ちください",
      isTransient: true,
      severity: "recoverable",
    };
  }

  // "Failed to lock state: {detail}" — lock contention; may resolve on retry
  // Emergency stop failing due to a lock error is critical because the user is stuck
  if (errorString.startsWith("Failed to lock state:")) {
    return {
      userMessage:
        context === "emergency_stop"
          ? "緊急停止に失敗しました。もう一度試してください"
          : "内部エラーが発生しました。もう一度試してください",
      isTransient: true,
      severity: context === "emergency_stop" ? "critical" : "recoverable",
    };
  }

  // "Window control error: {detail}" — window API failure, typically not transient
  if (errorString.startsWith("Window control error:")) {
    return {
      userMessage: "ウィンドウ操作に失敗しました",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Persistence error: {detail}" — file I/O or storage failure
  if (errorString.startsWith("Persistence error:")) {
    return {
      userMessage: "データの保存に失敗しました",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Input monitoring error: {detail}" — background listener failure
  if (errorString.startsWith("Input monitoring error:")) {
    return {
      userMessage: "入力モニタリングに失敗しました",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // "Unauthorized window: {detail}" — programming error, not transient
  if (errorString.startsWith("Unauthorized window:")) {
    return {
      userMessage: "権限エラーが発生しました",
      isTransient: false,
      severity: "recoverable",
    };
  }

  // Unknown error — treat as non-transient, recoverable by default
  return {
    userMessage: "予期しないエラーが発生しました",
    isTransient: false,
    severity: "recoverable",
  };
}
