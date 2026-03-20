import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AutoRunDayRow } from "../Settings/AutoRunDayRow";
import type { DayScheduleValidationError } from "../../types/weeklyAutoRun";

describe("AutoRunDayRow", () => {
  it("renders existing weekday row entry points with stable test ids", () => {
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
      screen.getByTestId("settings-auto-run-day-row-mon"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("settings-auto-run-day-enabled-mon"),
    ).toHaveAccessibleName("月曜日の自動実行を有効にする");
    expect(
      screen.getByTestId("settings-auto-run-day-start-mon"),
    ).toHaveAccessibleName("月曜日の開始時刻");
    expect(
      screen.getByTestId("settings-auto-run-day-end-mon"),
    ).toHaveAccessibleName("月曜日の終了時刻");
    expect(
      screen.getByTestId("settings-auto-run-day-overnight-mon"),
    ).toBeInTheDocument();
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
      screen.getByTestId("settings-auto-run-day-enabled-mon"),
    ).toBeDisabled();
    expect(
      screen.getByTestId("settings-auto-run-day-start-mon"),
    ).toBeDisabled();
    expect(screen.getByTestId("settings-auto-run-day-end-mon")).toBeDisabled();

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
      screen.getByTestId("settings-auto-run-day-enabled-mon"),
    ).toBeEnabled();
    expect(
      screen.getByTestId("settings-auto-run-day-start-mon"),
    ).toBeDisabled();
    expect(screen.getByTestId("settings-auto-run-day-end-mon")).toBeDisabled();
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

    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
      target: { value: "19:00" },
    });

    expect(onEnabledChange).toHaveBeenCalledWith(false);
    expect(onStartTimeChange).toHaveBeenCalledWith("10:00");
    expect(onEndTimeChange).toHaveBeenCalledWith("19:00");
  });

  it("shows overnight label and day validation message only when requested", () => {
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

    expect(
      screen.getByTestId("settings-auto-run-day-overnight-mon"),
    ).toHaveClass("text-primary-500");
    expect(
      screen.queryByTestId("settings-auto-run-day-error-mon"),
    ).not.toBeInTheDocument();

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

    expect(
      screen.getByTestId("settings-auto-run-day-overnight-mon"),
    ).toHaveClass("text-transparent");
    expect(
      screen.getByTestId("settings-auto-run-day-error-mon"),
    ).toHaveTextContent("Start time for mon must use HH:MM (24-hour).");

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

    expect(
      screen.queryByTestId("settings-auto-run-day-error-mon"),
    ).not.toBeInTheDocument();
  });
});
