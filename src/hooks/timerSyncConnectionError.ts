import { useErrorStore } from "@/store/errorStore";
import {
  classifyBackendError,
  formatBackendErrorDetail,
  formatPublicCommandErrorDetail,
} from "@/errors/classifyBackendError";

type TimerSyncConnectionFailureSource =
  | "setup"
  | "command"
  | "command_transport";

const SETUP_CONNECTION_ERROR_MESSAGE =
  "バックエンドに接続できません。アプリが正常に動作しない可能性があります。";

export function formatTimerSyncConnectionErrorDetail(
  error: unknown,
  source: TimerSyncConnectionFailureSource = "setup",
): string {
  if (source === "command" || source === "command_transport") {
    return formatPublicCommandErrorDetail(error);
  }

  return formatBackendErrorDetail(error);
}

export function reportTimerSyncConnectionError(
  error: unknown,
  retryFn: () => Promise<void>,
  source: TimerSyncConnectionFailureSource = "setup",
): void {
  const presentation =
    source === "command"
      ? classifyBackendError(error)
      : {
          userMessage: SETUP_CONNECTION_ERROR_MESSAGE,
          severity: "critical" as const,
          canRetry: true,
        };

  useErrorStore.getState().reportError({
    id: "connection",
    message: presentation.userMessage,
    technicalDetail: formatTimerSyncConnectionErrorDetail(error, source),
    severity: presentation.severity,
    origin: "backend_connection",
    canRetry: presentation.canRetry,
    retryFn: presentation.canRetry ? retryFn : null,
  });
}

export function clearTimerSyncConnectionError(): void {
  useErrorStore.getState().clearError("connection");
}
