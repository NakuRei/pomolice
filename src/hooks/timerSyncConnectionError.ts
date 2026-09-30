import { useErrorStore } from "@/store/errorStore";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
import {
  classifyCommandFailure,
  formatCommandFailureDetail,
  formatBackendErrorDetail,
} from "@/errors/classifyBackendError";

export type TimerSyncFailure =
  | CommandFailure
  | { status: "setup"; cause: unknown };

export function formatTimerSyncConnectionErrorDetail(
  failure: TimerSyncFailure,
): string {
  return failure.status === "setup"
    ? formatBackendErrorDetail(failure.cause)
    : formatCommandFailureDetail(failure);
}

export function reportTimerSyncConnectionError(
  failure: TimerSyncFailure,
  retryFn: () => Promise<void>,
): void {
  const presentation =
    failure.status === "rejected"
      ? classifyCommandFailure(failure)
      : {
          userMessage:
            "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
          severity: "critical" as const,
          canRetry: true,
        };
  useErrorStore.getState().reportError({
    id: "connection",
    message: presentation.userMessage,
    technicalDetail: formatTimerSyncConnectionErrorDetail(failure),
    severity: presentation.severity,
    origin: "backend_connection",
    canRetry: presentation.canRetry,
    retryFn: presentation.canRetry ? retryFn : null,
  });
}

export function clearTimerSyncConnectionError(): void {
  useErrorStore.getState().clearError("connection");
}
