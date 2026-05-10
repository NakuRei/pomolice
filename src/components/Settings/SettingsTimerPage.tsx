import { useState } from "react";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import { useSettingsStore, VALIDATION_RULES } from "@/store/settingsStore";
import { TimerStatus, useTimerStore } from "@/store/timerStore";
import { DurationInput } from "./DurationInput";
import {
  formatDurationInputText,
  parseDurationInputText,
} from "./durationInputValue";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";

interface SettingsTimerPageProps {
  openSettingsTopScreen: () => void;
}

interface TimerSettingDraftValues {
  workDuration: string;
  shortBreakDuration: string;
  longBreakDuration: string;
  longBreakInterval: string;
}

interface ValidTimerSettingValues {
  workDuration: number;
  shortBreakDuration: number;
  longBreakDuration: number;
  longBreakInterval: number;
}

function getValidTimerSettingValues(
  draftValues: TimerSettingDraftValues,
): ValidTimerSettingValues | null {
  const workDuration = parseDurationInputText(
    draftValues.workDuration,
    VALIDATION_RULES.workDuration.min,
    VALIDATION_RULES.workDuration.max,
    true,
  );
  const shortBreakDuration = parseDurationInputText(
    draftValues.shortBreakDuration,
    VALIDATION_RULES.shortBreakDuration.min,
    VALIDATION_RULES.shortBreakDuration.max,
    true,
  );
  const longBreakDuration = parseDurationInputText(
    draftValues.longBreakDuration,
    VALIDATION_RULES.longBreakDuration.min,
    VALIDATION_RULES.longBreakDuration.max,
    true,
  );
  const longBreakInterval = parseDurationInputText(
    draftValues.longBreakInterval,
    VALIDATION_RULES.longBreakInterval.min,
    VALIDATION_RULES.longBreakInterval.max,
    false,
  );

  if (
    workDuration === null ||
    shortBreakDuration === null ||
    longBreakDuration === null ||
    longBreakInterval === null
  ) {
    return null;
  }

  return {
    workDuration,
    shortBreakDuration,
    longBreakDuration,
    longBreakInterval,
  };
}

export function SettingsTimerPage({
  openSettingsTopScreen,
}: SettingsTimerPageProps): React.ReactElement {
  const headingReference = useFocusHeadingOnMount();

  const workDurationFromStore = useSettingsStore((state) => state.workDuration);
  const shortBreakDurationFromStore = useSettingsStore(
    (state) => state.shortBreakDuration,
  );
  const longBreakDurationFromStore = useSettingsStore(
    (state) => state.longBreakDuration,
  );
  const longBreakIntervalFromStore = useSettingsStore(
    (state) => state.longBreakInterval,
  );
  const updateSettings = useSettingsStore((state) => state.updateSettings);
  const timerStatus = useTimerStore((state) => state.status);

  const [workDurationDraft, setWorkDurationDraft] = useState<string>(
    formatDurationInputText(workDurationFromStore),
  );
  const [shortBreakDurationDraft, setShortBreakDurationDraft] =
    useState<string>(formatDurationInputText(shortBreakDurationFromStore));
  const [longBreakDurationDraft, setLongBreakDurationDraft] = useState<string>(
    formatDurationInputText(longBreakDurationFromStore),
  );
  const [longBreakIntervalDraft, setLongBreakIntervalDraft] = useState<string>(
    formatDurationInputText(longBreakIntervalFromStore),
  );

  const validTimerSettingValues = getValidTimerSettingValues({
    workDuration: workDurationDraft,
    shortBreakDuration: shortBreakDurationDraft,
    longBreakDuration: longBreakDurationDraft,
    longBreakInterval: longBreakIntervalDraft,
  });
  const timerDurationDraftValuesAreValid = validTimerSettingValues !== null;

  const isTimerIdle = timerStatus === TimerStatus.IDLE;

  const saveTimerSettingsPage = (): void => {
    if (validTimerSettingValues === null) {
      return;
    }

    updateSettings(validTimerSettingValues);

    openSettingsTopScreen();
  };

  const cancelTimerSettingsPage = (): void => {
    openSettingsTopScreen();
  };

  return (
    <section
      className="mx-auto w-full max-w-2xl"
      data-testid="settings-timer-page"
    >
      <SettingsPageHeader
        title="タイマー設定"
        headingRef={headingReference}
        headingTestId="settings-timer-heading"
        backButtonLabel="設定一覧へ戻る"
        backButtonTestId="settings-timer-back"
        onBackButtonClick={openSettingsTopScreen}
      />

      <div className="space-y-6 rounded-lg border border-border bg-surface p-4">
        <DurationInput
          label="作業時間"
          value={workDurationDraft}
          onChange={setWorkDurationDraft}
          min={VALIDATION_RULES.workDuration.min}
          max={VALIDATION_RULES.workDuration.max}
          testId="settings-work-duration"
          disabled={!isTimerIdle}
        />

        <DurationInput
          label="短い休憩"
          value={shortBreakDurationDraft}
          onChange={setShortBreakDurationDraft}
          min={VALIDATION_RULES.shortBreakDuration.min}
          max={VALIDATION_RULES.shortBreakDuration.max}
          testId="settings-short-break"
          disabled={!isTimerIdle}
        />

        <DurationInput
          label="長い休憩"
          value={longBreakDurationDraft}
          onChange={setLongBreakDurationDraft}
          min={VALIDATION_RULES.longBreakDuration.min}
          max={VALIDATION_RULES.longBreakDuration.max}
          testId="settings-long-break"
          disabled={!isTimerIdle}
        />

        <DurationInput
          label="長い休憩の間隔"
          value={longBreakIntervalDraft}
          onChange={setLongBreakIntervalDraft}
          min={VALIDATION_RULES.longBreakInterval.min}
          max={VALIDATION_RULES.longBreakInterval.max}
          unit="回"
          allowDecimal={false}
          testId="settings-interval"
          disabled={!isTimerIdle}
        />

        {!isTimerIdle && (
          <p
            className="text-xs text-warning-300"
            data-testid="settings-duration-edit-disabled-message"
          >
            タイマー実行中は時間長設定を変更できません。曜日別スケジュールのみ編集できます。
          </p>
        )}

        <SettingsSaveCancelFooter
          onCancelButtonClick={cancelTimerSettingsPage}
          onSaveButtonClick={saveTimerSettingsPage}
          saveButtonDisabled={!timerDurationDraftValuesAreValid}
          footerTestId="settings-timer-footer"
          footerClassName="pt-2"
        />
      </div>
    </section>
  );
}
