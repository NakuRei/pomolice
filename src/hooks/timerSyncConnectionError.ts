import { useErrorStore } from "@/store/errorStore";

export function reportTimerSyncConnectionError(
  error: unknown,
  retryFn: () => Promise<void>,
): void {
  useErrorStore.getState().reportError({
    id: "connection",
    message: "バックエンドに接続できません",
    technicalDetail: String(error),
    severity: "critical",
    origin: "backend_connection",
    canRetry: true,
    retryFn,
  });
}

export function clearTimerSyncConnectionError(): void {
  useErrorStore.getState().clearError("connection");
}
