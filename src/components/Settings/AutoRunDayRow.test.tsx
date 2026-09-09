import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AutoRunDayRow } from "./AutoRunDayRow";
import type { DayScheduleValidationError } from "@/types/weeklyAutoRun";

function getStartTimeInput(): HTMLElement {
  return screen.getByLabelText("月曜日の開始時刻");
}

function getEndTimeInput(): HTMLElement {
  return screen.getByLabelText("月曜日の終了時刻");
}

function renderEnabledMondayRow(
  dayValidationError?: DayScheduleValidationError,
): void {
  render(
    <AutoRunDayRow
      weekday="mon"
      autoRunFeatureEnabled={true}
      daySlot={{
        enabled: true,
        startTime: "09:00",
        endTime: "18:00",
      }}
      dayValidationError={dayValidationError}
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

  it("shows a day validation message only when requested", () => {
    const dayValidationError: DayScheduleValidationError = {
      code: "start_time_format_invalid",
      weekday: "mon",
      invalidStartTimeText: "invalid-time",
      message: "Start time for mon must use HH:MM (24-hour).",
    };

    const { rerender } = render(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "23:00",
          endTime: "02:00",
        }}
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
        dayValidationError={dayValidationError}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Start time for mon must use HH:MM (24-hour).",
    );

    rerender(
      <AutoRunDayRow
        weekday="mon"
        autoRunFeatureEnabled={true}
        daySlot={{
          enabled: true,
          startTime: "09:00",
          endTime: "18:00",
        }}
        onEnabledChange={() => undefined}
        onStartTimeChange={() => undefined}
        onEndTimeChange={() => undefined}
      />,
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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

  it("marks only the start time input invalid for start time format errors", () => {
    const dayValidationError: DayScheduleValidationError = {
      code: "start_time_format_invalid",
      weekday: "mon",
      invalidStartTimeText: "invalid-time",
      message: "Start time for mon must use HH:MM (24-hour).",
    };

    renderEnabledMondayRow(dayValidationError);

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();
    expect(startTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).toHaveAccessibleDescription(
      dayValidationError.message,
    );
    expect(endTimeInput).not.toHaveAccessibleDescription();
  });

  it("marks only the end time input invalid for end time format errors", () => {
    const dayValidationError: DayScheduleValidationError = {
      code: "end_time_format_invalid",
      weekday: "mon",
      invalidEndTimeText: "invalid-time",
      message: "End time for mon must use HH:MM (24-hour).",
    };

    renderEnabledMondayRow(dayValidationError);

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();
    expect(startTimeInput).not.toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).not.toHaveAccessibleDescription();
    expect(endTimeInput).toHaveAccessibleDescription(
      dayValidationError.message,
    );
  });

  it("marks both time inputs invalid when start and end times match", () => {
    const dayValidationError: DayScheduleValidationError = {
      code: "start_time_equals_end_time",
      weekday: "mon",
      startTime: "09:00",
      endTime: "09:00",
      message: "Start and end time cannot be identical for mon.",
    };

    renderEnabledMondayRow(dayValidationError);

    const startTimeInput = getStartTimeInput();
    const endTimeInput = getEndTimeInput();
    expect(startTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(endTimeInput).toHaveAttribute("aria-invalid", "true");
    expect(startTimeInput).toHaveAccessibleDescription(
      dayValidationError.message,
    );
    expect(endTimeInput).toHaveAccessibleDescription(
      dayValidationError.message,
    );
  });
});
