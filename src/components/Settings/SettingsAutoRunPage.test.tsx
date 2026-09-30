import type { SettingsInput, SettingsValidationIssue } from "@/ipc/generated";
import { answerSettingsSavesForTest } from "@/test/settingsSaveFixture";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

import {
  createSettingsForTest,
  createWeeklyAutoRunConfigForTest,
} from "@/test/settingsFixture";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import type { Settings } from "@/types/settings";
import { SettingsAutoRunPage } from "./SettingsAutoRunPage";

function acceptSettingsSaves(): void {
  mockInvoke.mockImplementation(answerSettingsSavesForTest);
}

function rejectSettingsSaves(issues: SettingsValidationIssue[]): void {
  mockInvoke.mockResolvedValue({
    status: "rejected",
    error: {
      code: "invalid_settings",
      message: "Invalid settings",
      issues,
    },
  });
}

function savedSettingsChanges(): Partial<SettingsInput>[] {
  return mockInvoke.mock.calls
    .filter(([command]) => command === "update_settings")
    .map(([, args]) => args?.changes as Partial<SettingsInput>);
}

function renderAutoRunPage(
  settings: Settings = createSettingsForTest(),
  openSettingsTopScreen = vi.fn(),
): ReturnType<typeof vi.fn> {
  render(
    <SettingsAutoRunPage
      settings={settings}
      openSettingsTopScreen={openSettingsTopScreen}
    />,
  );
  return openSettingsTopScreen;
}

describe("SettingsAutoRunPage", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    resetTimerStoreForTest();
    acceptSettingsSaves();
  });

  it("saves only the auto-run settings, with the whole week", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderAutoRunPage(
      createSettingsForTest({ workDurationSecs: 29 * 60 }),
    );

    await user.click(screen.getByRole("switch", { name: "曜日別自動実行" }));
    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    fireEvent.change(screen.getByLabelText("月曜日の開始時刻"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByLabelText("月曜日の終了時刻"), {
      target: { value: "19:00" },
    });
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
    });
    const [saved] = savedSettingsChanges();
    expect(saved?.auto_run_feature_enabled).toBe(true);
    expect(saved?.weekly_auto_run_config?.mon).toEqual({
      enabled: true,
      start_time: "10:00",
      end_time: "19:00",
    });
    expect(Object.keys(saved ?? {}).sort()).toEqual([
      "auto_run_feature_enabled",
      "weekly_auto_run_config",
    ]);
  });

  it("discards auto-run draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderAutoRunPage();

    await user.click(screen.getByRole("switch", { name: "曜日別自動実行" }));
    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(savedSettingsChanges()).toEqual([]);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("shows the backend's per-day issues on that day's row and stays on the page", async () => {
    const user = userEvent.setup();
    rejectSettingsSaves([
      { code: "empty_time_range", weekday: "mon" },
      { code: "invalid_time", weekday: "wed", boundary: "end" },
    ]);
    const openSettingsTopScreen = renderAutoRunPage(
      createSettingsForTest({ autoRunFeatureEnabled: true }),
    );

    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(
      await screen.findByTestId("settings-auto-run-day-error-mon"),
    ).toHaveTextContent("月曜日の開始時刻と終了時刻は別の時刻にしてください。");
    expect(
      screen.getByTestId("settings-auto-run-day-error-wed"),
    ).toHaveTextContent("水曜日の終了時刻を HH:MM 形式で入力してください。");
    expect(
      screen.queryByTestId("settings-auto-run-day-error-tue"),
    ).not.toBeInTheDocument();
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("lists every overlapping pair the backend reports in the weekly error notice", async () => {
    const user = userEvent.setup();
    rejectSettingsSaves([
      {
        code: "overlapping_time_ranges",
        primary_weekday: "mon",
        secondary_weekday: "tue",
      },
      {
        code: "overlapping_time_ranges",
        primary_weekday: "sat",
        secondary_weekday: "sun",
      },
    ]);
    renderAutoRunPage(
      createSettingsForTest({
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
          mon: { enabled: true, startTime: "23:00", endTime: "02:00" },
          tue: { enabled: true, startTime: "01:00", endTime: "03:00" },
        }),
      }),
    );

    await user.click(screen.getByRole("button", { name: "保存" }));

    const weeklyErrorNotice = await screen.findByTestId(
      "settings-auto-run-weekly-errors",
    );
    expect(weeklyErrorNotice).toHaveAttribute("role", "alert");
    expect(weeklyErrorNotice).toHaveTextContent(
      "月曜日と火曜日の時間帯が重なっています。",
    );
    expect(weeklyErrorNotice).toHaveTextContent(
      "土曜日と日曜日の時間帯が重なっています。",
    );
  });

  it("clears what the last save said about a day once that day is edited", async () => {
    const user = userEvent.setup();
    rejectSettingsSaves([
      { code: "empty_time_range", weekday: "mon" },
      { code: "invalid_time", weekday: "wed", boundary: "end" },
      {
        code: "overlapping_time_ranges",
        primary_weekday: "mon",
        secondary_weekday: "tue",
      },
    ]);
    renderAutoRunPage(
      createSettingsForTest({
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
          mon: { enabled: true, startTime: "10:00", endTime: "10:00" },
        }),
      }),
    );
    await user.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByTestId("settings-auto-run-day-error-mon");

    fireEvent.change(screen.getByLabelText("月曜日の終了時刻"), {
      target: { value: "12:00" },
    });

    expect(
      screen.queryByTestId("settings-auto-run-day-error-mon"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-auto-run-weekly-errors"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId("settings-auto-run-day-error-wed"),
    ).toBeInTheDocument();
  });

  it("shows no schedule errors before anything is saved", () => {
    renderAutoRunPage(
      createSettingsForTest({
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
          mon: { enabled: true, startTime: "23:00", endTime: "02:00" },
          tue: { enabled: true, startTime: "01:00", endTime: "03:00" },
        }),
      }),
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).not.toBeDisabled();
  });

  it("focuses the auto-run heading on mount", async () => {
    renderAutoRunPage();

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "曜日別自動実行設定" }),
      ).toHaveFocus();
    });
  });
});
