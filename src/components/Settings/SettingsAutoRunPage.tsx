import { useEffect, useRef, useState } from "react";
import { WEEKDAY_LABELS_BY_KEY } from "../../constants/weekdayLabels";
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
import { StatusNotice } from "../shared/StatusNotice";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";

interface SettingsAutoRunPageProps {
  openSettingsTopScreen: () => void;
}

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
      <SettingsPageHeader
        title="曜日別自動実行設定"
        headingRef={headingReference}
        headingTestId="settings-auto-run-heading"
        backButtonLabel="設定一覧へ戻る"
        backButtonTestId="settings-auto-run-back"
        onBackButtonClick={openSettingsTopScreen}
      />

      <section
        className="rounded-lg border border-border bg-surface p-3"
        data-testid="settings-auto-run-section"
      >
        <div className="mb-3 flex items-center justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-text">曜日別自動実行</h2>
            <p className="text-xs text-text-muted">
              曜日ごとに開始時刻と終了時刻を設定します。
            </p>
          </div>
          <label className="inline-flex items-center gap-2 text-sm text-text">
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
          <StatusNotice
            tone="danger"
            role="alert"
            className="mb-3"
            testId="settings-auto-run-weekly-errors"
          >
            <div className="space-y-1">
              {weeklyScheduleValidationErrors.map((weeklyValidationError) => (
                <p
                  key={`${weeklyValidationError.primaryWeekday}-${weeklyValidationError.secondaryWeekday}`}
                  className="text-xs text-danger-200"
                >
                  {weeklyValidationError.message}
                </p>
              ))}
            </div>
          </StatusNotice>
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
                ? "cursor-not-allowed border-disabled-border bg-disabled-bg text-disabled-text"
                : "border-border bg-surface-raised text-on-surface"
            }`;

            return (
              <div
                key={weekday}
                className={`rounded-md border px-2 py-2 transition-colors ${
                  dayTimeInputIsDisabled
                    ? "border-border bg-surface"
                    : "border-border bg-surface-raised"
                }`}
                data-testid={`settings-auto-run-day-row-${weekday}`}
              >
                <div className="grid grid-cols-[3rem_1fr_1fr_auto] items-center gap-2">
                  <label className="inline-flex items-center gap-1 text-xs text-text">
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
                      aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の自動実行を有効にする`}
                    />
                    <span>{WEEKDAY_LABELS_BY_KEY[weekday].short}</span>
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
                    aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の開始時刻`}
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
                    aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の終了時刻`}
                  />

                  <span
                    className={`text-xs ${
                      daySlotShowsOvernightLabel
                        ? "text-primary-500"
                        : "text-transparent"
                    }`}
                    data-testid={`settings-auto-run-day-overnight-${weekday}`}
                  >
                    翌日まで
                  </span>
                </div>
                {dayValidationError && (
                  <p
                    className="mt-1 text-xs text-danger-300"
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

      <SettingsSaveCancelFooter
        onCancelButtonClick={cancelAutoRunSettingsPage}
        onSaveButtonClick={saveAutoRunSettingsPage}
        saveButtonDisabled={autoRunDraftHasValidationErrors}
        footerTestId="settings-auto-run-footer"
        footerClassName="mt-4"
      />
    </section>
  );
}
