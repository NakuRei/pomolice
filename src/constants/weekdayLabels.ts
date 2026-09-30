import type { WeekdayKey } from "@/ipc/generated";

interface WeekdayLabelSet {
  short: string;
  full: string;
}

/**
 * Shared weekday labels for UI text.
 *
 * Usage example:
 * ```ts
 * WEEKDAY_LABELS_BY_KEY.mon.short; // "月"
 * WEEKDAY_LABELS_BY_KEY.mon.full; // "月曜日"
 * ```
 */
export const WEEKDAY_LABELS_BY_KEY: Record<WeekdayKey, WeekdayLabelSet> = {
  mon: { short: "月", full: "月曜日" },
  tue: { short: "火", full: "火曜日" },
  wed: { short: "水", full: "水曜日" },
  thu: { short: "木", full: "木曜日" },
  fri: { short: "金", full: "金曜日" },
  sat: { short: "土", full: "土曜日" },
  sun: { short: "日", full: "日曜日" },
};
