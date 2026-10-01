import type { PostBreakTransitionPolicy } from "@/ipc/generated";
import { executeOperation } from "@/backend/executeOperation";
import type { OperationResult } from "@/backend/operation";
import { useBackendObservationStore } from "@/backend/observationStore";
import type { SettingsChanges } from "../../backend/settings";

export function saveSettings(
  changes: SettingsChanges,
): Promise<OperationResult> {
  return executeOperation({ kind: "update_settings", changes });
}

export async function toggleNotificationSoundMuted(): Promise<
  OperationResult | undefined
> {
  const settings = useBackendObservationStore.getState().settings;
  if (settings === null) return undefined;
  return saveSettings({
    notificationSoundMuted: !settings.notificationSoundMuted,
  });
}

export function savePostBreakTransitionPolicy(
  policy: PostBreakTransitionPolicy,
): Promise<OperationResult> {
  return saveSettings({ postBreakTransitionPolicy: policy });
}
