import { useTimerStore } from "@/store/timerStore";
import { createTimerSnapshotForTest } from "./timerEffectsFixture";
import type { TimerSnapshot } from "@/types/timerEffects";
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
): Promise<TimerSnapshot> {
  if (command === "update_settings") {
    const { settings, handleSettingsChanged } = useTimerStore.getState();
    handleSettingsChanged({
      settings: settingsPayloadAfterChangesForTest(
        settings ?? createSettingsForTest(),
        args?.changes,
      ),
      settings_file_status: "ok",
    });
  }
  return Promise.resolve(createTimerSnapshotForTest());
}
