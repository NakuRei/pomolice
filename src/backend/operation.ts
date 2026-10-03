import type {
  CommandResponse,
  PostBreakDecisionAction,
  TimerSnapshot,
} from "@/ipc/generated";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
import type { SettingsChanges } from "@/backend/settings";

export type Operation =
  | { kind: "start_work" }
  | { kind: "emergency_stop" }
  | { kind: "resolve_post_break_decision"; action: PostBreakDecisionAction }
  | { kind: "set_away_during_break"; away: boolean }
  | { kind: "update_settings"; changes: SettingsChanges }
  | { kind: "resolve_boot_auto_start_prompt"; startNow: boolean }
  | { kind: "retry_failed_timer_effects" };

export type OperationKind = Operation["kind"];
export type OperationResult = CommandResponse<TimerSnapshot> | CommandFailure;
export type OperationOutcome =
  { status: "pending" } | { status: "accepted" } | CommandFailure;

export interface OperationRecord {
  id: number;
  request: Operation;
  outcome: OperationOutcome;
  dismissed: boolean;
}

export function getOperationFailure(
  outcome: OperationOutcome | undefined,
): CommandFailure | null {
  if (outcome?.status === "rejected" || outcome?.status === "unavailable")
    return outcome;
  return null;
}
