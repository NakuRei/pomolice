import { useEffect, useRef, useState } from "react";
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

export function SettingsTimerPage({
  openSettingsTopScreen,
}: SettingsTimerPageProps) {
  const headingReference = useRef<HTMLHeadingElement | null>(null);

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

  useEffect(() => {
    headingReference.current?.focus();
  }, []);

  const isWorkDurationValid =
    isFiniteNumberValue(workDurationDraft) &&
    workDurationDraft >= VALIDATION_RULES.workDuration.min &&
    workDurationDraft <= VALIDATION_RULES.workDuration.max;
  const isShortBreakDurationValid =
    isFiniteNumberValue(shortBreakDurationDraft) &&
    shortBreakDurationDraft >= VALIDATION_RULES.shortBreakDuration.min &&
    shortBreakDurationDraft <= VALIDATION_RULES.shortBreakDuration.max;
  const isLongBreakDurationValid =
    isFiniteNumberValue(longBreakDurationDraft) &&
    longBreakDurationDraft >= VALIDATION_RULES.longBreakDuration.min &&
    longBreakDurationDraft <= VALIDATION_RULES.longBreakDuration.max;
  const isLongBreakIntervalValid =
    isFiniteNumberValue(longBreakIntervalDraft) &&
    longBreakIntervalDraft >= VALIDATION_RULES.longBreakInterval.min &&
    longBreakIntervalDraft <= VALIDATION_RULES.longBreakInterval.max;

  const timerDurationDraftValuesAreValid =
    isWorkDurationValid &&
    isShortBreakDurationValid &&
    isLongBreakDurationValid &&
    isLongBreakIntervalValid &&
    workDurationDraft !== null &&
    shortBreakDurationDraft !== null &&
    longBreakDurationDraft !== null &&
    longBreakIntervalDraft !== null;

  const isTimerIdle = timerStatus === TimerStatus.IDLE;

  const saveTimerSettingsPage = () => {
    if (!timerDurationDraftValuesAreValid) {
      return;
    }

    updateSettings({
      workDuration: workDurationDraft,
      shortBreakDuration: shortBreakDurationDraft,
      longBreakDuration: longBreakDurationDraft,
      longBreakInterval: longBreakIntervalDraft,
    });

    openSettingsTopScreen();
  };

  const cancelTimerSettingsPage = () => {
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

      <div className="space-y-6">
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
          testId="settings-interval"
          disabled={!isTimerIdle}
        />

        {!isTimerIdle && (
          <p
            className="text-xs text-amber-700"
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
