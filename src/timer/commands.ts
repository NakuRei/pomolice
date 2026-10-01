import { useTimerObservationStore } from "@/store/timerObservationStore";
import {
  commands,
  type CommandResponse,
  type TimerSnapshot,
  type PostBreakDecisionAction,
  type PostBreakTransitionPolicy,
} from "@/ipc/generated";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
import { useTimerOperationStore } from "@/store/timerOperationStore";
import {
  settingsChangesToPayload,
  type SettingsChanges,
} from "@/types/settings";
import type { TimerCommandResult, TimerOperation } from "./operation";

async function executeTimerCommand(
  request: TimerOperation,
  send: () => Promise<CommandResponse<TimerSnapshot>>,
): Promise<TimerCommandResult> {
  const id = useTimerOperationStore.getState().begin(request);
  const result = await receiveCommandResponse(send());
  if (result.status === "accepted")
    useTimerObservationStore.getState().observeEffects(result.value.effects);
  useTimerOperationStore
    .getState()
    .complete(
      request.kind,
      id,
      result.status === "accepted" ? { status: "accepted" } : result,
    );
  return result;
}
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
export function saveSettings(
  changes: SettingsChanges,
): Promise<TimerCommandResult> {
  const submitted = structuredClone(changes);
  return executeTimerCommand(
    { kind: "update_settings", changes: submitted },
    () => commands.updateSettings(settingsChangesToPayload(submitted)),
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
export async function toggleNotificationSoundMuted(): Promise<
  TimerCommandResult | undefined
> {
  const settings = useTimerObservationStore.getState().settings;
  if (settings === null) return undefined;
  return saveSettings({
    notificationSoundMuted: !settings.notificationSoundMuted,
  });
}
export function savePostBreakTransitionPolicy(
  policy: PostBreakTransitionPolicy,
): Promise<TimerCommandResult> {
  return saveSettings({ postBreakTransitionPolicy: policy });
}
