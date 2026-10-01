import { commands, type PostBreakTransitionPolicy } from "@/ipc/generated";
import { executeTimerCommand } from "@/operations/executeTimerCommand";
import type { TimerCommandResult } from "@/operations/operation";
import { useTimerObservationStore } from "@/store/timerObservationStore";
import { settingsChangesToPayload, type SettingsChanges } from "./settings";

export function saveSettings(
  changes: SettingsChanges,
): Promise<TimerCommandResult> {
  const submitted = structuredClone(changes);
  return executeTimerCommand(
    { kind: "update_settings", changes: submitted },
    () => commands.updateSettings(settingsChangesToPayload(submitted)),
  );
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
