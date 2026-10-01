import { useTimerObservationStore } from "@/store/timerObservationStore";
import { acceptedCommand } from "./commandResponseFixture";
import type { TimerSnapshot, CommandResponse } from "@/ipc/generated";
import { createTimerSnapshotForTest } from "./timerEffectsFixture";

import {
  createSettingsForTest,
  settingsPayloadAfterChangesForTest,
} from "./settingsFixture";

/**
 * Answers commands the way the backend does for a settings save: the saved
 * settings arrive as a settings_changed event before the operation outcome.
 */
export function answerSettingsSavesForTest(
  command: string,
  args?: Record<string, unknown>,
): Promise<CommandResponse<TimerSnapshot>> {
  if (command === "update_settings") {
    const { settings, observeSettingsChanged } =
      useTimerObservationStore.getState();
    observeSettingsChanged({
      settings: settingsPayloadAfterChangesForTest(
        settings ?? createSettingsForTest(),
        args?.changes,
      ),
      settings_file_status: "ok",
    });
  }
  return Promise.resolve(acceptedCommand(createTimerSnapshotForTest()));
}
