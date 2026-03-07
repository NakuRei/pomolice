import { useEffect, useRef, useState } from "react";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import { TimerStatus, useTimerStore } from "../../store/timerStore";
import { DurationInput } from "./DurationInput";

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
      <header className="mb-5">
        <h1
          ref={headingReference}
          tabIndex={-1}
          data-testid="settings-timer-heading"
          className="text-2xl font-bold text-gray-900"
        >
          タイマー設定
        </h1>
      </header>

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

        <footer className="flex gap-3 pt-2" data-testid="settings-timer-footer">
          <button
            type="button"
            onClick={cancelTimerSettingsPage}
            className="flex-1 rounded-lg border border-gray-300 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-gray-400"
            data-testid="settings-cancel"
          >
            キャンセル
          </button>
          <button
            type="button"
            onClick={saveTimerSettingsPage}
            disabled={!timerDurationDraftValuesAreValid}
            className={`flex-1 rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 ${
              timerDurationDraftValuesAreValid
                ? "bg-blue-600 hover:bg-blue-700 focus-visible:outline-blue-500"
                : "cursor-not-allowed bg-gray-400 focus-visible:outline-gray-400"
            }`}
            data-testid="settings-save"
          >
            保存
          </button>
        </footer>
      </div>
    </section>
  );
}
