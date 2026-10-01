import type { PostBreakDecisionAction } from "@/ipc/generated";
import { executeOperation } from "@/backend/executeOperation";
import type { OperationResult } from "@/backend/operation";

export function startWork(): Promise<OperationResult> {
  return executeOperation({ kind: "start_work" });
}
export function emergencyStop(): Promise<OperationResult> {
  return executeOperation({ kind: "emergency_stop" });
}
export function resolvePostBreakDecision(
  action: PostBreakDecisionAction,
): Promise<OperationResult> {
  return executeOperation({ kind: "resolve_post_break_decision", action });
}
export function setAwayDuringBreak(away: boolean): Promise<OperationResult> {
  return executeOperation({ kind: "set_away_during_break", away });
}
export function resolveBootAutoStartPrompt(
  startNow: boolean,
): Promise<OperationResult> {
  return executeOperation({ kind: "resolve_boot_auto_start_prompt", startNow });
}
