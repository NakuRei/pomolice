import type { ErrorSeverity } from "../store/errorStore";

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

const EXACT_MATCH_CLASSIFICATIONS: Readonly<
  Partial<Record<string, BackendErrorClassification>>
> = {
  "Timer already running": {
    userMessage: "すでに実行中です",
    isTransient: false,
    severity: "recoverable",
  },
  "Timer not running": {
    userMessage: "タイマーが停止しています",
    isTransient: false,
    severity: "recoverable",
  },
};

interface BackendErrorPrefixRule {
  prefix: string;
  classify: (context: CommandContext) => BackendErrorClassification;
}

const PREFIX_MATCH_CLASSIFICATIONS: readonly BackendErrorPrefixRule[] = [
  {
    prefix: "Invalid state transition:",
    classify: () => ({
      userMessage: "現在の状態では実行できません",
      isTransient: false,
      severity: "recoverable",
    }),
  },
  {
    prefix: "Rate limited:",
    classify: () => ({
      userMessage: "少々お待ちください",
      isTransient: true,
      severity: "recoverable",
    }),
  },
  {
    prefix: "Failed to lock state:",
    classify: (context) => ({
      userMessage:
        context === "emergency_stop"
          ? "緊急停止に失敗しました。もう一度試してください"
          : "内部エラーが発生しました。もう一度試してください",
      isTransient: true,
      severity: context === "emergency_stop" ? "critical" : "recoverable",
    }),
  },
  {
    prefix: "Window control error:",
    classify: () => ({
      userMessage: "ウィンドウ操作に失敗しました",
      isTransient: false,
      severity: "recoverable",
    }),
  },
  {
    prefix: "Persistence error:",
    classify: () => ({
      userMessage: "データの保存に失敗しました",
      isTransient: false,
      severity: "recoverable",
    }),
  },
  {
    prefix: "Input monitoring error:",
    classify: () => ({
      userMessage: "入力モニタリングに失敗しました",
      isTransient: false,
      severity: "recoverable",
    }),
  },
  {
    prefix: "Unauthorized window:",
    classify: () => ({
      userMessage: "権限エラーが発生しました",
      isTransient: false,
      severity: "recoverable",
    }),
  },
];

const DEFAULT_BACKEND_ERROR_CLASSIFICATION: BackendErrorClassification = {
  userMessage: "予期しないエラーが発生しました",
  isTransient: false,
  severity: "recoverable",
};

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
  context: CommandContext,
): BackendErrorClassification {
  const exactMatchClassification = EXACT_MATCH_CLASSIFICATIONS[errorString];
  if (exactMatchClassification) {
    return exactMatchClassification;
  }

  const prefixMatchRule = PREFIX_MATCH_CLASSIFICATIONS.find(({ prefix }) =>
    errorString.startsWith(prefix),
  );
  if (prefixMatchRule) {
    return prefixMatchRule.classify(context);
  }

  return DEFAULT_BACKEND_ERROR_CLASSIFICATION;
}
