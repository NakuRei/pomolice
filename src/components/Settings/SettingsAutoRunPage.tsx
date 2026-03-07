import { useEffect, useRef, useState } from "react";
import {
  useSettingsStore,
  WEEKDAY_KEYS,
  parseHhMm,
  validateWeeklyAutoRunConfig,
  type DayScheduleValidationError,
  type WeeklyAutoRunConfig,
  type WeeklyScheduleValidationError,
  type WeekdayKey,
} from "../../store/settingsStore";

interface SettingsAutoRunPageProps {
  openSettingsTopScreen: () => void;
}

const WEEKDAY_LABEL_BY_KEY: Record<WeekdayKey, string> = {
  mon: "月",
  tue: "火",
  wed: "水",
  thu: "木",
  fri: "金",
  sat: "土",
  sun: "日",
};

function cloneWeeklyAutoRunConfigForDraft(
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
): WeeklyAutoRunConfig {
  return {
    mon: { ...weeklyAutoRunConfig.mon },
    tue: { ...weeklyAutoRunConfig.tue },
    wed: { ...weeklyAutoRunConfig.wed },
    thu: { ...weeklyAutoRunConfig.thu },
    fri: { ...weeklyAutoRunConfig.fri },
    sat: { ...weeklyAutoRunConfig.sat },
    sun: { ...weeklyAutoRunConfig.sun },
  };
}

function daySlotCrossesMidnight(
  daySlot: WeeklyAutoRunConfig[WeekdayKey],
): boolean {
  const parsedStartTime = parseHhMm(daySlot.startTime);
  const parsedEndTime = parseHhMm(daySlot.endTime);
  if (!parsedStartTime || !parsedEndTime) {
    return false;
  }
  return (
    parsedStartTime.minutesSinceStartOfDay >
    parsedEndTime.minutesSinceStartOfDay
  );
}

export function SettingsAutoRunPage({
  openSettingsTopScreen,
}: SettingsAutoRunPageProps) {
  const headingReference = useRef<HTMLHeadingElement | null>(null);

  const autoRunFeatureEnabledFromStore = useSettingsStore(
    (state) => state.autoRunFeatureEnabled,
  );
  const weeklyAutoRunConfigFromStore = useSettingsStore(
    (state) => state.weeklyAutoRunConfig,
  );
  const updateSettings = useSettingsStore((state) => state.updateSettings);

  const weeklyAutoRunDraftInitialValue = cloneWeeklyAutoRunConfigForDraft(
    weeklyAutoRunConfigFromStore,
  );
  const validationResultAtPageOpen = validateWeeklyAutoRunConfig(
    weeklyAutoRunDraftInitialValue,
  );

  const [autoRunFeatureEnabledDraft, setAutoRunFeatureEnabledDraft] =
    useState<boolean>(autoRunFeatureEnabledFromStore);
  const [weeklyAutoRunConfigDraft, setWeeklyAutoRunConfigDraft] =
    useState<WeeklyAutoRunConfig>(weeklyAutoRunDraftInitialValue);
  const [dayScheduleValidationErrors, setDayScheduleValidationErrors] =
    useState<Partial<Record<WeekdayKey, DayScheduleValidationError>>>(
      validationResultAtPageOpen.dayScheduleValidationErrors,
    );
  const [weeklyScheduleValidationErrors, setWeeklyScheduleValidationErrors] =
    useState<WeeklyScheduleValidationError[]>(
      validationResultAtPageOpen.weeklyScheduleValidationErrors,
    );

  useEffect(() => {
    headingReference.current?.focus();
  }, []);

  const updateWeeklyAutoRunDraft = (
    updateDraftSchedule: (
      draftWeeklyAutoRunConfig: WeeklyAutoRunConfig,
    ) => void,
  ) => {
    setWeeklyAutoRunConfigDraft((previousDraft) => {
      const nextDraft = cloneWeeklyAutoRunConfigForDraft(previousDraft);
      updateDraftSchedule(nextDraft);

      const draftValidationResult = validateWeeklyAutoRunConfig(nextDraft);
      setDayScheduleValidationErrors(
        draftValidationResult.dayScheduleValidationErrors,
      );
      setWeeklyScheduleValidationErrors(
        draftValidationResult.weeklyScheduleValidationErrors,
      );

      return nextDraft;
    });
  };

  const autoRunDraftHasValidationErrors =
    Object.keys(dayScheduleValidationErrors).length > 0 ||
    weeklyScheduleValidationErrors.length > 0;

  const saveAutoRunSettingsPage = () => {
    const validationResultBeforeSave = validateWeeklyAutoRunConfig(
      weeklyAutoRunConfigDraft,
    );
    setDayScheduleValidationErrors(
      validationResultBeforeSave.dayScheduleValidationErrors,
    );
    setWeeklyScheduleValidationErrors(
      validationResultBeforeSave.weeklyScheduleValidationErrors,
    );
    if (
      Object.keys(validationResultBeforeSave.dayScheduleValidationErrors)
        .length > 0 ||
      validationResultBeforeSave.weeklyScheduleValidationErrors.length > 0
    ) {
      return;
    }

    updateSettings({
      autoRunFeatureEnabled: autoRunFeatureEnabledDraft,
      weeklyAutoRunConfig: weeklyAutoRunConfigDraft,
    });
    openSettingsTopScreen();
  };

  const cancelAutoRunSettingsPage = () => {
    openSettingsTopScreen();
  };

  return (
    <section
      className="mx-auto w-full max-w-2xl"
      data-testid="settings-auto-run-page"
    >
      <header className="mb-5">
        <h1
          ref={headingReference}
          tabIndex={-1}
          data-testid="settings-auto-run-heading"
          className="text-2xl font-bold text-gray-900"
        >
          曜日別自動実行設定
        </h1>
      </header>

      <section
        className="rounded-lg border border-gray-200 p-3"
        data-testid="settings-auto-run-section"
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-gray-800">
              曜日別自動実行
            </h2>
            <p className="text-xs text-gray-600">
              曜日ごとに開始時刻と終了時刻を設定します。
            </p>
          </div>
          <label className="inline-flex items-center gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={autoRunFeatureEnabledDraft}
              onChange={(event) =>
                setAutoRunFeatureEnabledDraft(event.target.checked)
              }
              data-testid="settings-auto-run-feature-toggle"
              aria-label="曜日別自動実行の全体切り替え"
            />
            有効化
          </label>
        </div>

        {weeklyScheduleValidationErrors.length > 0 && (
          <div
            className="mb-3 rounded-md border border-red-200 bg-red-50 p-2"
            data-testid="settings-auto-run-weekly-errors"
            role="alert"
          >
            {weeklyScheduleValidationErrors.map((weeklyValidationError) => (
              <p
                key={`${weeklyValidationError.primaryWeekday}-${weeklyValidationError.secondaryWeekday}`}
                className="text-xs text-red-700"
              >
                {weeklyValidationError.message}
              </p>
            ))}
          </div>
        )}

        <div className="grid gap-2">
          {WEEKDAY_KEYS.map((weekday) => {
            const daySlot = weeklyAutoRunConfigDraft[weekday];
            const dayValidationError = dayScheduleValidationErrors[weekday];
            const daySlotShowsOvernightLabel =
              daySlot.enabled && daySlotCrossesMidnight(daySlot);
            const dayTimeInputIsDisabled =
              !autoRunFeatureEnabledDraft || !daySlot.enabled;
            const dayTimeInputClassName = `rounded border px-2 py-1 text-sm transition-colors ${
              dayTimeInputIsDisabled
                ? "cursor-not-allowed border-gray-200 bg-gray-100 text-gray-400"
                : "border-gray-300 bg-white text-gray-900"
            }`;

            return (
              <div
                key={weekday}
                className={`rounded-md border px-2 py-2 transition-colors ${
                  dayTimeInputIsDisabled
                    ? "border-gray-100 bg-gray-50"
                    : "border-gray-100 bg-white"
                }`}
                data-testid={`settings-auto-run-day-row-${weekday}`}
              >
                <div className="grid grid-cols-[3rem_1fr_1fr_auto] items-center gap-2">
                  <label className="inline-flex items-center gap-1 text-xs text-gray-700">
                    <input
                      type="checkbox"
                      checked={daySlot.enabled}
                      disabled={!autoRunFeatureEnabledDraft}
                      onChange={(event) =>
                        updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                          draftWeeklyAutoRunConfig[weekday].enabled =
                            event.target.checked;
                        })
                      }
                      data-testid={`settings-auto-run-day-enabled-${weekday}`}
                      aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の自動実行を有効にする`}
                    />
                    <span>{WEEKDAY_LABEL_BY_KEY[weekday]}</span>
                  </label>

                  <input
                    type="time"
                    step={60}
                    value={daySlot.startTime}
                    onChange={(event) =>
                      updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                        draftWeeklyAutoRunConfig[weekday].startTime =
                          event.target.value;
                      })
                    }
                    disabled={dayTimeInputIsDisabled}
                    className={dayTimeInputClassName}
                    data-testid={`settings-auto-run-day-start-${weekday}`}
                    aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の開始時刻`}
                  />

                  <input
                    type="time"
                    step={60}
                    value={daySlot.endTime}
                    onChange={(event) =>
                      updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                        draftWeeklyAutoRunConfig[weekday].endTime =
                          event.target.value;
                      })
                    }
                    disabled={dayTimeInputIsDisabled}
                    className={dayTimeInputClassName}
                    data-testid={`settings-auto-run-day-end-${weekday}`}
                    aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の終了時刻`}
                  />

                  <span
                    className={`text-xs ${
                      daySlotShowsOvernightLabel
                        ? "text-blue-700"
                        : "text-transparent"
                    }`}
                    data-testid={`settings-auto-run-day-overnight-${weekday}`}
                  >
                    翌日まで
                  </span>
                </div>
                {dayValidationError && (
                  <p
                    className="mt-1 text-xs text-red-700"
                    data-testid={`settings-auto-run-day-error-${weekday}`}
                    role="alert"
                  >
                    {dayValidationError.message}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      </section>

      <footer
        className="mt-4 flex gap-3"
        data-testid="settings-auto-run-footer"
      >
        <button
          type="button"
          onClick={cancelAutoRunSettingsPage}
          className="flex-1 rounded-lg border border-gray-300 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-gray-400"
          data-testid="settings-cancel"
        >
          キャンセル
        </button>
        <button
          type="button"
          onClick={saveAutoRunSettingsPage}
          disabled={autoRunDraftHasValidationErrors}
          className={`flex-1 rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 ${
            autoRunDraftHasValidationErrors
              ? "cursor-not-allowed bg-gray-400 focus-visible:outline-gray-400"
              : "bg-blue-600 hover:bg-blue-700 focus-visible:outline-blue-500"
          }`}
          data-testid="settings-save"
        >
          保存
        </button>
      </footer>
    </section>
  );
}
