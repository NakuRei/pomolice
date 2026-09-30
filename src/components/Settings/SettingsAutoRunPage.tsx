import type { SettingsValidationIssue, WeekdayKey } from "@/ipc/generated";
import { useId, useState } from "react";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import { useSettingsSave } from "@/hooks/useSettingsSave";
import type { Settings } from "@/types/settings";
import { WEEKDAY_KEYS, type WeeklyAutoRunConfig } from "@/types/weeklyAutoRun";
import { StatusNotice } from "../shared/StatusNotice";
import { SwitchButton } from "../shared/SwitchButton";
import { AutoRunDayRow } from "./AutoRunDayRow";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";
import {
  overlapIssueMessages,
  summarizeDayIssues,
} from "./settingsIssueMessages";

/** Editing a weekday settles what the last save said about that day. */
function issueConcernsWeekday(
  issue: SettingsValidationIssue,
  weekday: WeekdayKey,
): boolean {
  if (issue.code === "invalid_time" || issue.code === "empty_time_range") {
    return issue.weekday === weekday;
  }
  if (issue.code === "overlapping_time_ranges") {
    return (
      issue.primary_weekday === weekday || issue.secondary_weekday === weekday
    );
  }
  return false;
}

interface SettingsAutoRunPageProps {
  settings: Settings;
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
  settings,
  openSettingsTopScreen,
}: SettingsAutoRunPageProps): React.ReactElement {
  const headingReference = useFocusHeadingOnMount();
  const autoRunSectionHeadingId = useId();
  const { issues, isSaving, save, settleIssues } = useSettingsSave(
    openSettingsTopScreen,
  );

  const [autoRunFeatureEnabledDraft, setAutoRunFeatureEnabledDraft] =
    useState<boolean>(settings.autoRunFeatureEnabled);
  const [weeklyAutoRunConfigDraft, setWeeklyAutoRunConfigDraft] =
    useState<WeeklyAutoRunConfig>(() =>
      cloneWeeklyAutoRunConfigForDraft(settings.weeklyAutoRunConfig),
    );

  const updateWeeklyAutoRunDraft = (
    weekday: WeekdayKey,
    updateDraftSchedule: (
      draftWeeklyAutoRunConfig: WeeklyAutoRunConfig,
    ) => void,
  ): void => {
    setWeeklyAutoRunConfigDraft((previousDraft) => {
      const nextDraft = cloneWeeklyAutoRunConfigForDraft(previousDraft);
      updateDraftSchedule(nextDraft);
      return nextDraft;
    });
    settleIssues((issue) => issueConcernsWeekday(issue, weekday));
  };

  const overlapMessages = overlapIssueMessages(issues);

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
            <h2
              id={autoRunSectionHeadingId}
              className="text-sm font-semibold text-text"
            >
              曜日別自動実行
            </h2>
            <p className="text-xs text-text-muted">
              曜日ごとに開始時刻と終了時刻を設定します。
            </p>
          </div>
          <SwitchButton
            aria-labelledby={autoRunSectionHeadingId}
            checked={autoRunFeatureEnabledDraft}
            onCheckedChange={setAutoRunFeatureEnabledDraft}
          />
        </div>

        {overlapMessages.length > 0 && (
          <StatusNotice
            tone="danger"
            role="alert"
            className="mb-3"
            testId="settings-auto-run-weekly-errors"
          >
            <div className="space-y-1">
              {overlapMessages.map(({ key, message }) => (
                <p key={key} className="text-xs text-danger-200">
                  {message}
                </p>
              ))}
            </div>
          </StatusNotice>
        )}

        <div className="grid gap-2">
          {WEEKDAY_KEYS.map((weekday) => {
            const daySlot = weeklyAutoRunConfigDraft[weekday];

            return (
              <AutoRunDayRow
                key={weekday}
                weekday={weekday}
                autoRunFeatureEnabled={autoRunFeatureEnabledDraft}
                daySlot={daySlot}
                dayIssues={summarizeDayIssues(issues, weekday)}
                onEnabledChange={(enabled) => {
                  updateWeeklyAutoRunDraft(
                    weekday,
                    (draftWeeklyAutoRunConfig) => {
                      draftWeeklyAutoRunConfig[weekday].enabled = enabled;
                    },
                  );
                }}
                onStartTimeChange={(timeText) => {
                  updateWeeklyAutoRunDraft(
                    weekday,
                    (draftWeeklyAutoRunConfig) => {
                      draftWeeklyAutoRunConfig[weekday].startTime = timeText;
                    },
                  );
                }}
                onEndTimeChange={(timeText) => {
                  updateWeeklyAutoRunDraft(
                    weekday,
                    (draftWeeklyAutoRunConfig) => {
                      draftWeeklyAutoRunConfig[weekday].endTime = timeText;
                    },
                  );
                }}
              />
            );
          })}
        </div>
      </section>

      <SettingsSaveCancelFooter
        onCancelButtonClick={openSettingsTopScreen}
        onSaveButtonClick={() => {
          void save({
            autoRunFeatureEnabled: autoRunFeatureEnabledDraft,
            weeklyAutoRunConfig: weeklyAutoRunConfigDraft,
          });
        }}
        saveButtonDisabled={isSaving}
        footerTestId="settings-auto-run-footer"
        footerClassName="mt-4"
      />
    </section>
  );
}
