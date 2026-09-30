import type {
  SettingsValidationIssue,
  BackendErrorCode,
  BackendErrorResponse,
} from "@/ipc/generated";
import type { ErrorSeverity } from "../store/errorStore";

/** The result of classifying a structured backend error response. */
export interface BackendErrorClassification {
  /** User-facing presentation identity shown in the UI. */
  userMessage: string;
  /** Display importance used to choose the UI treatment. */
  severity: ErrorSeverity;
  /** Whether the UI should offer a retry action. */
  canRetry: boolean;
}

const CODE_CLASSIFICATIONS: Readonly<
  Record<BackendErrorCode, BackendErrorClassification>
> = {
  unauthorized_window: {
    userMessage: "権限エラーが発生しました",
    severity: "recoverable",
    canRetry: false,
  },
  invalid_state_transition: {
    userMessage: "現在の状態では実行できません",
    severity: "recoverable",
    canRetry: false,
  },
  rate_limited: {
    userMessage: "少々お待ちください",
    severity: "recoverable",
    canRetry: true,
  },
  invalid_settings: {
    userMessage: "設定内容を確認してください",
    severity: "recoverable",
    canRetry: false,
  },
  window_control_error: {
    userMessage: "ウィンドウ操作に失敗しました",
    severity: "recoverable",
    canRetry: true,
  },
  persistence_error: {
    userMessage: "データの保存に失敗しました",
    severity: "recoverable",
    canRetry: true,
  },
  invalid_command_input: {
    userMessage: "入力内容を確認してください",
    severity: "recoverable",
    canRetry: false,
  },
};

const DEFAULT_BACKEND_ERROR_CLASSIFICATION: BackendErrorClassification = {
  userMessage: "予期しないエラーが発生しました",
  severity: "recoverable",
  canRetry: false,
};

const BACKEND_ERROR_CODES: ReadonlySet<string> = new Set(
  Object.keys(CODE_CLASSIFICATIONS),
);

function isBackendErrorCode(code: unknown): code is BackendErrorCode {
  return typeof code === "string" && BACKEND_ERROR_CODES.has(code);
}

export function isBackendErrorResponse(
  error: unknown,
): error is BackendErrorResponse {
  if (typeof error !== "object" || error === null) {
    return false;
  }

  const response = error as Record<string, unknown>;
  return (
    isBackendErrorCode(response.code) && typeof response.message === "string"
  );
}

/**
 * The issues of a rejected settings save, or `null` for any other error.
 * A rejection without issues names nothing to fix, so it is treated as an
 * ordinary failure rather than as a rejection.
 */
export function settingsValidationIssuesOf(
  error: unknown,
): SettingsValidationIssue[] | null {
  if (!isBackendErrorResponse(error) || error.code !== "invalid_settings") {
    return null;
  }

  const { issues } = error as BackendErrorResponse & { issues?: unknown };
  return Array.isArray(issues) && issues.length > 0 ? issues : null;
}

export function formatBackendErrorDetail(error: unknown): string {
  if (isBackendErrorResponse(error)) {
    return `${error.code}: ${error.message}`;
  }

  if (error instanceof Error) {
    return String(error);
  }

  if (typeof error === "string") {
    return error;
  }

  return "Unexpected backend error";
}

export function formatPublicCommandErrorDetail(error: unknown): string {
  if (isBackendErrorResponse(error)) {
    return error.code;
  }

  return "Unexpected backend error";
}

/** Map structured backend error responses to user-visible classifications. */
export function classifyBackendError(
  error: unknown,
): BackendErrorClassification {
  if (isBackendErrorResponse(error)) {
    return CODE_CLASSIFICATIONS[error.code];
  }

  return DEFAULT_BACKEND_ERROR_CLASSIFICATION;
}
