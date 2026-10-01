import type { BackendErrorResponse } from "@/ipc/generated";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
export type ErrorSeverity = "critical" | "recoverable" | "silent";

export interface BackendErrorClassification {
  userMessage: string;
  severity: ErrorSeverity;
  canRetry: boolean;
}

const CODE_CLASSIFICATIONS: Readonly<
  Record<BackendErrorResponse["code"], BackendErrorClassification>
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
};

export function classifyCommandFailure(
  failure: CommandFailure,
): BackendErrorClassification {
  return failure.status === "rejected"
    ? CODE_CLASSIFICATIONS[failure.error.code]
    : {
        userMessage: "操作の結果を確認できませんでした",
        severity: "recoverable",
        canRetry: false,
      };
}

export function formatCommandFailureDetail(failure: CommandFailure): string {
  return failure.status === "rejected"
    ? failure.error.code
    : "Command response unavailable";
}

export function formatBackendErrorDetail(error: unknown): string {
  if (error instanceof Error) return String(error);
  if (typeof error === "string") return error;
  return "Unexpected backend error";
}
