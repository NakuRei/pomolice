import { useState } from "react";
import { useFocusHeadingOnMount } from "../../hooks/useFocusHeadingOnMount";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import { TimerStatus, useTimerStore } from "../../store/timerStore";
import { DurationInput } from "./DurationInput";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";

interface SettingsTimerPageProps {
  openSettingsTopScreen: () => void;
}

const isFiniteNumberValue = (value: number | null): value is number =>
  typeof value === "number" && Number.isFinite(value);

const isNumberWithinRange = (
  value: number | null,
  min: number,
  max: number,
): value is number =>
  isFiniteNumberValue(value) && value >= min && value <= max;

const isWholeNumberWithinRange = (
  value: number | null,
  min: number,
  max: number,
): value is number =>
  isNumberWithinRange(value, min, max) && Number.isInteger(value);

interface TimerSettingDraftValues {
  workDuration: number | null;
  shortBreakDuration: number | null;
  longBreakDuration: number | null;
  longBreakInterval: number | null;
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
  const {
    workDuration,
    shortBreakDuration,
    longBreakDuration,
    longBreakInterval,
  } = draftValues;

  if (
    !isNumberWithinRange(
      workDuration,
      VALIDATION_RULES.workDuration.min,
      VALIDATION_RULES.workDuration.max,
    )
  ) {
    return null;
  }

  if (
    !isNumberWithinRange(
      shortBreakDuration,
      VALIDATION_RULES.shortBreakDuration.min,
      VALIDATION_RULES.shortBreakDuration.max,
    )
  ) {
    return null;
  }

  if (
    !isNumberWithinRange(
      longBreakDuration,
      VALIDATION_RULES.longBreakDuration.min,
      VALIDATION_RULES.longBreakDuration.max,
    )
  ) {
    return null;
  }

  if (
    !isWholeNumberWithinRange(
      longBreakInterval,
      VALIDATION_RULES.longBreakInterval.min,
      VALIDATION_RULES.longBreakInterval.max,
    )
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

  const [workDurationDraft, setWorkDurationDraft] = useState<number | null>(
    workDurationFromStore,
  );
  const [shortBreakDurationDraft, setShortBreakDurationDraft] = useState<
    number | null
  >(shortBreakDurationFromStore);
  const [longBreakDurationDraft, setLongBreakDurationDraft] = useState<
    number | null
  >(longBreakDurationFromStore);
  const [longBreakIntervalDraft, setLongBreakIntervalDraft] = useState<
    number | null
  >(longBreakIntervalFromStore);

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
