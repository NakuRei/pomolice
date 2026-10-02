import { useBackendObservationStore } from "@/backend/observationStore";
import { acceptedCommand } from "./commandResponseFixture";
import type { TimerSnapshot, CommandResponse } from "@/ipc/generated";
import { createTimerSnapshotForTest } from "./timerEffectsFixture";

import {
  createSettingsForTest,
  settingsPayloadAfterChangesForTest,
} from "./settingsFixture";

export function answerSettingsSavesForTest(
  command: string,
  args?: Record<string, unknown>,
): Promise<CommandResponse<TimerSnapshot>> {
  const snapshot = createTimerSnapshotForTest({
    revision: String(
      BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
    ),
  });
  if (command === "update_settings") {
    const { settings } = useBackendObservationStore.getState();
    snapshot.timer = {
      ...snapshot.timer,
      settings: settingsPayloadAfterChangesForTest(
        settings ?? createSettingsForTest(),
        args?.changes,
      ),
      settings_file_status: "ok",
    };
  }
  return Promise.resolve(acceptedCommand(snapshot));
}
