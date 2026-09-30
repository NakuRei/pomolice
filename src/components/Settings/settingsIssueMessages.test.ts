import type {
  RangedSettingField,
  SettingsValidationIssue,
} from "@/ipc/generated";
import { describe, expect, it } from "vitest";

import {
  overlapIssueMessages,
  rangedFieldIssueMessage,
  summarizeDayIssues,
} from "./settingsIssueMessages";

describe("rangedFieldIssueMessage", () => {
  it.each([
    {
      field: "work_duration_secs",
      min: 1,
      max: 7200,
      expected: "1秒〜120分の範囲で入力してください。",
    },
    {
      field: "long_break_duration_secs",
      min: 90,
      max: 3600,
      expected: "1分30秒〜60分の範囲で入力してください。",
    },
    {
      field: "long_break_interval",
      min: 2,
      max: 10,
      expected: "2〜10回の範囲で入力してください。",
    },
    {
      field: "notification_sound_volume_percent",
      min: 0,
      max: 100,
      expected: "0〜100%の範囲で入力してください。",
    },
  ] satisfies {
    field: RangedSettingField;
    min: number;
    max: number;
    expected: string;
  }[])(
    "describes the allowed range of $field in its own unit",
    ({ field, min, max, expected }) => {
      expect(
        rangedFieldIssueMessage(
          [{ code: "out_of_range", field, min, max }],
          field,
        ),
      ).toBe(expected);
    },
  );

  it("returns nothing for a field the backend did not reject", () => {
    const issues: SettingsValidationIssue[] = [
      { code: "out_of_range", field: "long_break_interval", min: 2, max: 10 },
    ];

    expect(
      rangedFieldIssueMessage(issues, "work_duration_secs"),
    ).toBeUndefined();
  });
});

describe("summarizeDayIssues", () => {
  it("collects only the given weekday's issues and marks the affected inputs", () => {
    const issues: SettingsValidationIssue[] = [
      { code: "invalid_time", weekday: "tue", boundary: "end" },
      { code: "empty_time_range", weekday: "mon" },
      {
        code: "overlapping_time_ranges",
        primary_weekday: "tue",
        secondary_weekday: "wed",
      },
    ];

    expect(summarizeDayIssues(issues, "tue")).toEqual({
      messages: ["火曜日の終了時刻を HH:MM 形式で入力してください。"],
      startTimeIsInvalid: false,
      endTimeIsInvalid: true,
    });
  });

  it("reports nothing for a weekday without issues", () => {
    expect(summarizeDayIssues([], "sun")).toEqual({
      messages: [],
      startTimeIsInvalid: false,
      endTimeIsInvalid: false,
    });
  });
});

describe("overlapIssueMessages", () => {
  it("names both weekdays of every overlapping pair", () => {
    const issues: SettingsValidationIssue[] = [
      {
        code: "overlapping_time_ranges",
        primary_weekday: "mon",
        secondary_weekday: "sun",
      },
      { code: "empty_time_range", weekday: "wed" },
      {
        code: "overlapping_time_ranges",
        primary_weekday: "fri",
        secondary_weekday: "sat",
      },
    ];

    expect(overlapIssueMessages(issues).map(({ message }) => message)).toEqual([
      "月曜日と日曜日の時間帯が重なっています。",
      "金曜日と土曜日の時間帯が重なっています。",
    ]);
  });
});
