import { WEEKDAY_LABELS_BY_KEY } from "../../constants/weekdayLabels";
import {
  type DayAutoRunSlot,
  type DayScheduleValidationError,
  type WeekdayKey,
} from "../../types/weeklyAutoRun";

interface AutoRunDayRowProps {
  weekday: WeekdayKey;
  autoRunFeatureEnabled: boolean;
  daySlot: DayAutoRunSlot;
  dayValidationError?: DayScheduleValidationError;
  onEnabledChange: (enabled: boolean) => void;
  onStartTimeChange: (timeText: string) => void;
  onEndTimeChange: (timeText: string) => void;
}

const TIME_TEXT_PATTERN = /^(?:[01]\d|2[0-3]):[0-5]\d$/;

function parseTimeToMinutesSinceStartOfDay(timeText: string): number | null {
  if (!TIME_TEXT_PATTERN.test(timeText)) {
    return null;
  }

  const [hourText, minuteText] = timeText.split(":");
  return Number(hourText) * 60 + Number(minuteText);
}

function daySlotCrossesMidnight(daySlot: DayAutoRunSlot): boolean {
  const startMinutesSinceStartOfDay = parseTimeToMinutesSinceStartOfDay(
    daySlot.startTime,
  );
  const endMinutesSinceStartOfDay = parseTimeToMinutesSinceStartOfDay(
    daySlot.endTime,
  );
  if (
    startMinutesSinceStartOfDay === null ||
    endMinutesSinceStartOfDay === null
  ) {
    return false;
  }

  return startMinutesSinceStartOfDay > endMinutesSinceStartOfDay;
}

export function AutoRunDayRow({
  weekday,
  autoRunFeatureEnabled,
  daySlot,
  dayValidationError,
  onEnabledChange,
  onStartTimeChange,
  onEndTimeChange,
}: AutoRunDayRowProps) {
  const daySlotShowsOvernightLabel =
    daySlot.enabled && daySlotCrossesMidnight(daySlot);
  const dayTimeInputIsDisabled = !autoRunFeatureEnabled || !daySlot.enabled;
  const dayTimeInputClassName = `rounded border px-2 py-1 text-sm transition-colors ${
    dayTimeInputIsDisabled
      ? "cursor-not-allowed border-disabled-border bg-disabled-bg text-disabled-text"
      : "border-border bg-surface-raised text-on-surface"
  }`;

  return (
    <div
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
            disabled={!autoRunFeatureEnabled}
            onChange={(event) => onEnabledChange(event.target.checked)}
            data-testid={`settings-auto-run-day-enabled-${weekday}`}
            aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の自動実行を有効にする`}
          />
          <span>{WEEKDAY_LABELS_BY_KEY[weekday].short}</span>
        </label>

        <input
          type="time"
          step={60}
          value={daySlot.startTime}
          onChange={(event) => onStartTimeChange(event.target.value)}
          disabled={dayTimeInputIsDisabled}
          className={dayTimeInputClassName}
          data-testid={`settings-auto-run-day-start-${weekday}`}
          aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の開始時刻`}
        />

        <input
          type="time"
          step={60}
          value={daySlot.endTime}
          onChange={(event) => onEndTimeChange(event.target.value)}
          disabled={dayTimeInputIsDisabled}
          className={dayTimeInputClassName}
          data-testid={`settings-auto-run-day-end-${weekday}`}
          aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の終了時刻`}
        />

        <span
          className={`text-xs ${
            daySlotShowsOvernightLabel ? "text-primary-500" : "text-transparent"
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
}
