import { useErrorStore } from "@/store/errorStore";
import { formatBackendErrorDetail } from "@/errors/classifyBackendError";

export function reportTimerSyncConnectionError(
  error: unknown,
  retryFn: () => Promise<void>,
): void {
  useErrorStore.getState().reportError({
    id: "connection",
    message: "バックエンドに接続できません",
    technicalDetail: formatBackendErrorDetail(error),
    severity: "critical",
    origin: "backend_connection",
    canRetry: true,
    retryFn,
  });
}

export function clearTimerSyncConnectionError(): void {
  useErrorStore.getState().clearError("connection");
}
