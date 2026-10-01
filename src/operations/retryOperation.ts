import { useTimerObservationStore } from "@/store/timerObservationStore";
import type { OperationRecord, TimerOperation } from "./operation";
import { useTimerOperationStore } from "./operationStore";
import { classifyCommandFailure } from "@/errors/classifyBackendError";
import {
  startWork,
  emergencyStop,
  resolvePostBreakDecision,
  setAwayDuringBreak,
  resolveBootAutoStartPrompt,
  retryFailedEffects,
} from "@/timer/commands";
import { saveSettings } from "@/features/settings/commands";

export function canRetryOperation(
  record: OperationRecord,
  hasBootOffer: boolean,
): boolean {
  if (record.outcome.status !== "rejected") return false;
  return (
    classifyCommandFailure(record.outcome).canRetry &&
    (record.request.kind !== "resolve_boot_auto_start_prompt" || hasBootOffer)
  );
}

async function resendRejectedRequest(request: TimerOperation): Promise<void> {
  switch (request.kind) {
    case "start_work":
      await startWork();
      return;
    case "emergency_stop":
      await emergencyStop();
      return;
    case "resolve_post_break_decision":
      await resolvePostBreakDecision(request.action);
      return;
    case "set_away_during_break":
      await setAwayDuringBreak(request.away);
      return;
    case "update_settings":
      await saveSettings(request.changes);
      return;
    case "resolve_boot_auto_start_prompt":
      await resolveBootAutoStartPrompt(request.startNow);
      return;
    case "retry_failed_timer_effects":
      await retryFailedEffects();
      return;
    default: {
      const unreachable: never = request;
      throw new Error(`Unknown timer operation: ${String(unreachable)}`);
    }
  }
}

export async function retryRejectedOperation(
  record: OperationRecord,
): Promise<void> {
  const current =
    useTimerOperationStore.getState().operations[record.request.kind];
  if (
    current?.id !== record.id ||
    !canRetryOperation(
      current,
      Boolean(useTimerObservationStore.getState().bootAutoStartOffer),
    )
  )
    return;
  await resendRejectedRequest(current.request);
}
