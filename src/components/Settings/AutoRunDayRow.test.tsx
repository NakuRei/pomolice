import type { SettingsValidationIssue } from "@/ipc/generated";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AutoRunDayRow } from "./AutoRunDayRow";
import {
  summarizeDayIssues,
  type DayIssueSummary,
} from "./settingsIssueMessages";

const NO_DAY_ISSUES: DayIssueSummary = summarizeDayIssues([], "mon");

function mondayIssues(...issues: SettingsValidationIssue[]): DayIssueSummary {
  return summarizeDayIssues(issues, "mon");
}

function getStartTimeInput(): HTMLElement {
  return screen.getByLabelText("月曜日の開始時刻");
}

function getEndTimeInput(): HTMLElement {
  return screen.getByLabelText("月曜日の終了時刻");
}

function renderEnabledMondayRow(dayIssues = NO_DAY_ISSUES): void {
  render(
    <AutoRunDayRow
      weekday="mon"
      autoRunFeatureEnabled={true}
      daySlot={{
        enabled: true,
        startTime: "09:00",
        endTime: "18:00",
      }}
      dayIssues={dayIssues}
      onEnabledChange={() => undefined}
      onStartTimeChange={() => undefined}
      onEndTimeChange={() => undefined}
    />,
  );
}

describe("AutoRunDayRow", () => {
  it("renders weekday controls with accessible names", () => {
    render(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={NO_DAY_ISSUES}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("月曜日の開始時刻")).toBeInTheDocument();
    expect(screen.getByLabelText("月曜日の終了時刻")).toBeInTheDocument();
  });

  it("disables row controls according to feature and weekday state", () => {
    const { rerender } = render(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={false}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={NO_DAY_ISSUES}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    ).toBeDisabled();
    expect(screen.getByLabelText("月曜日の開始時刻")).toBeDisabled();
    expect(screen.getByLabelText("月曜日の終了時刻")).toBeDisabled();

    rerender(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: false,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={NO_DAY_ISSUES}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    ).toBeEnabled();
    expect(screen.getByLabelText("月曜日の開始時刻")).toBeDisabled();
    expect(screen.getByLabelText("月曜日の終了時刻")).toBeDisabled();
  });

  it("calls row callbacks when enabled, start time, and end time change", async () => {
    const user = userEvent.setup();
    const onEnabledChange = vi.fn();
    const onStartTimeChange = vi.fn();
    const onEndTimeChange = vi.fn();

    render(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={NO_DAY_ISSUES}
        onEnabledChange={onEnabledChange}
        onStartTimeChange={onStartTimeChange}
        onEndTimeChange={onEndTimeChange}
      />,
    );

    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    fireEvent.change(screen.getByLabelText("月曜日の開始時刻"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByLabelText("月曜日の終了時刻"), {
      target: { value: "19:00" },
    });

    expect(onEnabledChange).toHaveBeenCalledWith(false);
    expect(onStartTimeChange).toHaveBeenCalledWith("10:00");
    expect(onEndTimeChange).toHaveBeenCalledWith("19:00");
  });

  it("shows the day's issue messages only when there are issues", () => {
    const { rerender } = render(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={NO_DAY_ISSUES}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    rerender(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        dayIssues={mondayIssues({
          code: "invalid_time",
          weekday: "mon",
          boundary: "start",
        })}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "月曜日の開始時刻を HH:MM 形式で入力してください。",
    );
  });

  it("does not announce validation errors when the day has no error", () => {
    renderEnabledMondayRow();

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();

    expect(startTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).not.toHaveAccessibleDescription();
    expect(endTimeInput).not.toHaveAccessibleDescription();
  });

  it("marks only the start time input invalid for an unusable start time", () => {
    renderEnabledMondayRow(
      mondayIssues({ code: "invalid_time", weekday: "mon", boundary: "start" }),
    );

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();
    expect(startTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).toHaveAccessibleDescription(
      "月曜日の開始時刻を HH:MM 形式で入力してください。",
    );
    expect(endTimeInput).not.toHaveAccessibleDescription();
  });

  it("marks only the end time input invalid for an unusable end time", () => {
    renderEnabledMondayRow(
      mondayIssues({ code: "invalid_time", weekday: "mon", boundary: "end" }),
    );

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();
    expect(startTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).not.toHaveAccessibleDescription();
    expect(endTimeInput).toHaveAccessibleDescription(
      "月曜日の終了時刻を HH:MM 形式で入力してください。",
    );
  });

  it("marks both time inputs invalid when start and end times match", () => {
    renderEnabledMondayRow(
      mondayIssues({ code: "empty_time_range", weekday: "mon" }),
    );

    const expectedMessage =
      "月曜日の開始時刻と終了時刻は別の時刻にしてください。";
    expect(getStartTimeInput()).toHaveAttribute("aria-invalid", "true");
    expect(getEndTimeInput()).toHaveAttribute("aria-invalid", "true");
    expect(getStartTimeInput()).toHaveAccessibleDescription(expectedMessage);
    expect(getEndTimeInput()).toHaveAccessibleDescription(expectedMessage);
  });
});
