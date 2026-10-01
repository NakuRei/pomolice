import { useTimerObservationStore } from "@/store/timerObservationStore";
import { commands, type PostBreakDecisionAction } from "@/ipc/generated";
import { executeTimerCommand } from "@/operations/executeTimerCommand";
import type { TimerCommandResult } from "@/operations/operation";

export function startWork(): Promise<TimerCommandResult> {
  return executeTimerCommand({ kind: "start_work" }, commands.startWork);
}
export function emergencyStop(): Promise<TimerCommandResult> {
  return executeTimerCommand(
    { kind: "emergency_stop" },
    commands.emergencyStop,
  );
}
export async function resolvePostBreakDecision(
  action: PostBreakDecisionAction,
): Promise<TimerCommandResult> {
  const result = await executeTimerCommand(
    { kind: "resolve_post_break_decision", action },
    () => commands.resolvePostBreakDecision(action),
  );
  if (result.status === "accepted")
    useTimerObservationStore.getState().closePostBreakDecision();
  return result;
}
export function setAwayDuringBreak(away: boolean): Promise<TimerCommandResult> {
  return executeTimerCommand({ kind: "set_away_during_break", away }, () =>
    commands.setAwayDuringBreak(away),
  );
}
export async function resolveBootAutoStartPrompt(
  startNow: boolean,
): Promise<TimerCommandResult> {
  const result = await executeTimerCommand(
    { kind: "resolve_boot_auto_start_prompt", startNow },
    () => commands.resolveBootAutoStartPrompt(startNow),
  );
  if (result.status === "accepted")
    useTimerObservationStore.getState().clearBootAutoStartOffer();
  return result;
}
let pendingEffectRetry: Promise<TimerCommandResult> | null = null;
export function retryFailedEffects(): Promise<TimerCommandResult> {
  pendingEffectRetry ??= executeTimerCommand(
    { kind: "retry_failed_timer_effects" },
    commands.retryFailedTimerEffects,
  ).finally(() => {
    pendingEffectRetry = null;
  });
  return pendingEffectRetry;
}
