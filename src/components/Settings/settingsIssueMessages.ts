import { formatDurationText } from "@/components/shared/durationText";
import { WEEKDAY_LABELS_BY_KEY } from "@/constants/weekdayLabels";
import type {
  RangedSettingField,
  SettingsValidationIssue,
} from "@/types/settings";
import type { WeekdayKey } from "@/types/weeklyAutoRun";

const RANGE_UNIT_BY_FIELD: Record<RangedSettingField, "duration" | "回" | "%"> =
  {
    work_duration_secs: "duration",
    short_break_duration_secs: "duration",
    long_break_duration_secs: "duration",
    long_break_interval: "回",
    notification_sound_volume_percent: "%",
  };

function formatRange(
  field: RangedSettingField,
  min: number,
  max: number,
): string {
  const unit = RANGE_UNIT_BY_FIELD[field];
  if (unit === "duration") {
    return `${formatDurationText(min)}〜${formatDurationText(max)}`;
  }
  return `${String(min)}〜${String(max)}${unit}`;
}

/** The message for a rejected numeric field, if the backend rejected it. */
export function rangedFieldIssueMessage(
  issues: readonly SettingsValidationIssue[],
  field: RangedSettingField,
): string | undefined {
  const issue = issues.find(
    (candidate) =>
      candidate.code === "out_of_range" && candidate.field === field,
  );
  if (issue?.code !== "out_of_range") {
    return undefined;
  }

  return `${formatRange(field, issue.min, issue.max)}の範囲で入力してください。`;
}

export interface DayIssueSummary {
  messages: string[];
  startTimeIsInvalid: boolean;
  endTimeIsInvalid: boolean;
}

/** Issues that concern one weekday on its own. */
export function summarizeDayIssues(
  issues: readonly SettingsValidationIssue[],
  weekday: WeekdayKey,
): DayIssueSummary {
  const dayLabel = WEEKDAY_LABELS_BY_KEY[weekday].full;
  const summary: DayIssueSummary = {
    messages: [],
    startTimeIsInvalid: false,
    endTimeIsInvalid: false,
  };

  for (const issue of issues) {
    if (issue.code === "invalid_time" && issue.weekday === weekday) {
      const boundaryLabel = issue.boundary === "start" ? "開始" : "終了";
      summary.messages.push(
        `${dayLabel}の${boundaryLabel}時刻を HH:MM 形式で入力してください。`,
      );
      if (issue.boundary === "start") {
        summary.startTimeIsInvalid = true;
      } else {
        summary.endTimeIsInvalid = true;
      }
    }
    if (issue.code === "empty_time_range" && issue.weekday === weekday) {
      summary.messages.push(
        `${dayLabel}の開始時刻と終了時刻は別の時刻にしてください。`,
      );
      summary.startTimeIsInvalid = true;
      summary.endTimeIsInvalid = true;
    }
  }

  return summary;
}

/** One message per pair of weekdays whose time ranges overlap. */
export function overlapIssueMessages(
  issues: readonly SettingsValidationIssue[],
): { key: string; message: string }[] {
  return issues.flatMap((issue) =>
    issue.code === "overlapping_time_ranges"
      ? [
          {
            key: `${issue.primary_weekday}-${issue.secondary_weekday}`,
            message: `${WEEKDAY_LABELS_BY_KEY[issue.primary_weekday].full}と${WEEKDAY_LABELS_BY_KEY[issue.secondary_weekday].full}の時間帯が重なっています。`,
          },
        ]
      : [],
  );
}
