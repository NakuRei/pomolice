import type { TimerSyncFailure } from "@/store/timerSyncStore";
import {
  classifyCommandFailure,
  formatCommandFailureDetail,
  formatBackendErrorDetail,
} from "@/errors/classifyBackendError";
export function formatTimerSyncConnectionErrorDetail(
  failure: TimerSyncFailure,
): string {
  return failure.status === "setup"
    ? formatBackendErrorDetail(failure.cause)
    : formatCommandFailureDetail(failure);
}
export function describeTimerSyncFailure(failure: TimerSyncFailure): {
  userMessage: string;
  canRetry: boolean;
} {
  return failure.status === "rejected"
    ? classifyCommandFailure(failure)
    : {
        userMessage:
          "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
        canRetry: true,
      };
}
