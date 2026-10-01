import { useBackendObservationStore } from "@/backend/observationStore";
import type { RangedSettingField } from "@/ipc/generated";
import { useState } from "react";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import { useSettingsSave } from "../useSettingsSave";

import type { Settings, SettingsChanges } from "../../../backend/settings";
import { DurationInput } from "./DurationInput";
import {
  formatDurationInputText,
  parseDurationInputText,
} from "./durationInputValue";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";
import { rangedFieldIssueMessage } from "./settingsIssueMessages";

interface SettingsTimerPageProps {
  settings: Settings;
  openSettingsTopScreen: () => void;
}

type TimerSettingField = Extract<
  RangedSettingField,
  | "work_duration_secs"
  | "short_break_duration_secs"
  | "long_break_duration_secs"
  | "long_break_interval"
>;

type TimerSettingDraftValues = Record<TimerSettingField, string>;

const SECONDS_PER_MINUTE = 60;

function formatMinutesInputText(seconds: number): string {
  return formatDurationInputText(
    Math.round((seconds / SECONDS_PER_MINUTE) * 100) / 100,
  );
}

function parseMinutesToSeconds(raw: string): number | null {
  const minutes = parseDurationInputText(raw, true);
  return minutes === null ? null : Math.round(minutes * SECONDS_PER_MINUTE);
}

function createDraftValues(settings: Settings): TimerSettingDraftValues {
  return {
    work_duration_secs: formatMinutesInputText(settings.workDurationSecs),
    short_break_duration_secs: formatMinutesInputText(
      settings.shortBreakDurationSecs,
    ),
    long_break_duration_secs: formatMinutesInputText(
      settings.longBreakDurationSecs,
    ),
    long_break_interval: formatDurationInputText(settings.longBreakInterval),
  };
}

function changesFromDraftValues(
  draftValues: TimerSettingDraftValues,
): SettingsChanges | null {
  const workDurationSecs = parseMinutesToSeconds(
    draftValues.work_duration_secs,
  );
  const shortBreakDurationSecs = parseMinutesToSeconds(
    draftValues.short_break_duration_secs,
  );
  const longBreakDurationSecs = parseMinutesToSeconds(
    draftValues.long_break_duration_secs,
  );
  const longBreakInterval = parseDurationInputText(
    draftValues.long_break_interval,
    false,
  );

  if (
    workDurationSecs === null ||
    shortBreakDurationSecs === null ||
    longBreakDurationSecs === null ||
    longBreakInterval === null
  ) {
    return null;
  }

  return {
    workDurationSecs,
    shortBreakDurationSecs,
    longBreakDurationSecs,
    longBreakInterval,
  };
}

export function SettingsTimerPage({
  settings,
  openSettingsTopScreen,
}: SettingsTimerPageProps): React.ReactElement {
  const headingReference = useFocusHeadingOnMount();
  const timerStatus = useBackendObservationStore((state) => state.status);
  const { issues, isSaving, save, settleIssues } = useSettingsSave(
    openSettingsTopScreen,
  );

  const [draftValues, setDraftValues] = useState<TimerSettingDraftValues>(() =>
    createDraftValues(settings),
  );

  const changes = changesFromDraftValues(draftValues);

  const updateDraftValue = (field: TimerSettingField, value: string): void => {
    setDraftValues((previous) => ({ ...previous, [field]: value }));
    settleIssues(
      (issue) => issue.code === "out_of_range" && issue.field === field,
    );
  };

  const renderDurationInput = (
    field: TimerSettingField,
    label: string,
    options: { unit?: string; allowDecimal?: boolean; testId: string },
  ): React.ReactElement => (
    <DurationInput
      label={label}
      value={draftValues[field]}
      onChange={(value) => {
        updateDraftValue(field, value);
      }}
      unit={options.unit}
      allowDecimal={options.allowDecimal}
      testId={options.testId}
      errorMessage={rangedFieldIssueMessage(issues, field)}
    />
  );

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
        {renderDurationInput("work_duration_secs", "作業時間", {
          testId: "settings-work-duration",
        })}
        {renderDurationInput("short_break_duration_secs", "短い休憩", {
          testId: "settings-short-break",
        })}
        {renderDurationInput("long_break_duration_secs", "長い休憩", {
          testId: "settings-long-break",
        })}
        {renderDurationInput("long_break_interval", "長い休憩の間隔", {
          unit: "回",
          allowDecimal: false,
          testId: "settings-interval",
        })}

        {(timerStatus === "working" || timerStatus === "on_break") && (
          <p
            className="text-xs text-text-muted"
            data-testid="settings-duration-applies-next-session"
          >
            実行中のセッションの長さは変わりません。保存した時間は次のセッションから使われます。
          </p>
        )}

        <SettingsSaveCancelFooter
          onCancelButtonClick={openSettingsTopScreen}
          onSaveButtonClick={() => {
            if (changes !== null) {
              void save(changes);
            }
          }}
          saveButtonDisabled={changes === null || isSaving}
          footerTestId="settings-timer-footer"
          footerClassName="pt-2"
        />
      </div>
    </section>
  );
}
