import { useState } from "react";
import { useFocusHeadingOnMount } from "../../hooks/useFocusHeadingOnMount";
import {
  useSettingsStore,
  WEEKDAY_KEYS,
  validateWeeklyAutoRunConfig,
  type DayScheduleValidationError,
  type WeeklyAutoRunConfig,
  type WeeklyScheduleValidationError,
  type WeekdayKey,
} from "../../store/settingsStore";
import { StatusNotice } from "../shared/StatusNotice";
import { AutoRunDayRow } from "./AutoRunDayRow";
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

export function SettingsAutoRunPage({
  openSettingsTopScreen,
}: SettingsAutoRunPageProps): React.ReactElement {
  const headingReference = useFocusHeadingOnMount();

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

  const updateWeeklyAutoRunDraft = (
    updateDraftSchedule: (
      draftWeeklyAutoRunConfig: WeeklyAutoRunConfig,
    ) => void,
  ): void => {
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

  const saveAutoRunSettingsPage = (): void => {
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

  const cancelAutoRunSettingsPage = (): void => {
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
              onChange={(event) => {
                setAutoRunFeatureEnabledDraft(event.target.checked);
              }}
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

            return (
              <AutoRunDayRow
                key={weekday}
                weekday={weekday}
                autoRunFeatureEnabled={autoRunFeatureEnabledDraft}
                daySlot={daySlot}
                dayValidationError={dayValidationError}
                onEnabledChange={(enabled) => {
                  updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                    draftWeeklyAutoRunConfig[weekday].enabled = enabled;
                  });
                }}
                onStartTimeChange={(timeText) => {
                  updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                    draftWeeklyAutoRunConfig[weekday].startTime = timeText;
                  });
                }}
                onEndTimeChange={(timeText) => {
                  updateWeeklyAutoRunDraft((draftWeeklyAutoRunConfig) => {
                    draftWeeklyAutoRunConfig[weekday].endTime = timeText;
                  });
                }}
              />
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
