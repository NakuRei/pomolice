import { useTimerObservationStore } from "@/store/timerObservationStore";
import type { SettingsInput } from "@/ipc/generated";
import { answerSettingsSavesForTest } from "@/test/settingsSaveFixture";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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

import { createSettingsForTest } from "@/test/settingsFixture";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import type { Settings } from "../settings";
import { SettingsTimerPage } from "./SettingsTimerPage";

function acceptSettingsSaves(): void {
  mockInvoke.mockImplementation(answerSettingsSavesForTest);
}

function savedSettingsChanges(): Partial<SettingsInput>[] {
  return mockInvoke.mock.calls
    .filter(([command]) => command === "update_settings")
    .map(([, args]) => args?.changes as Partial<SettingsInput>);
}

function renderTimerPage(
  settings: Settings = createSettingsForTest(),
  openSettingsTopScreen = vi.fn(),
): ReturnType<typeof vi.fn> {
  render(
    <SettingsTimerPage
      settings={settings}
      openSettingsTopScreen={openSettingsTopScreen}
    />,
  );
  return openSettingsTopScreen;
}

describe("SettingsTimerPage", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    resetTimerStoreForTest();
    acceptSettingsSaves();
  });

  it("saves only the timer durations, in seconds", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderTimerPage(
      createSettingsForTest({
        autoRunFeatureEnabled: true,
        notificationSoundVolumePercent: 70,
      }),
    );

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "32");
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
    });
    expect(savedSettingsChanges()).toEqual([
      {
        work_duration_secs: 32 * 60,
        short_break_duration_secs: 5 * 60,
        long_break_duration_secs: 15 * 60,
        long_break_interval: 4,
      },
    ]);
  });

  it("shows the saved durations in minutes", () => {
    renderTimerPage(
      createSettingsForTest({
        workDurationSecs: 50 * 60,
        shortBreakDurationSecs: 90,
      }),
    );

    expect(screen.getByRole("spinbutton", { name: "作業時間" })).toHaveValue(
      50,
    );
    expect(screen.getByRole("spinbutton", { name: "短い休憩" })).toHaveValue(
      1.5,
    );
  });

  it("discards timer draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderTimerPage();

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "44");
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(savedSettingsChanges()).toEqual([]);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("blocks timer save when long break interval draft is not an integer", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderTimerPage();

    const intervalInput = screen.getByRole("spinbutton", {
      name: "長い休憩の間隔",
    });
    await user.clear(intervalInput);
    await user.type(intervalInput, "2.5");

    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(savedSettingsChanges()).toEqual([]);
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("shows the backend's range error next to the rejected field and stays on the page", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({
      status: "rejected",
      error: {
        code: "invalid_settings",
        message: "Invalid settings",
        issues: [
          {
            code: "out_of_range",
            field: "work_duration_secs",
            min: 1,
            max: 7200,
          },
        ],
      },
    });
    const openSettingsTopScreen = renderTimerPage();

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "999");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(
      await screen.findByText("1秒〜120分の範囲で入力してください。"),
    ).toBeInTheDocument();
    expect(workInput).toHaveAttribute("aria-invalid", "true");
    expect(workInput).toHaveValue(999);
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("keeps inputs editable while the timer runs and says the change applies from the next session", () => {
    useTimerObservationStore.setState({ status: "working" });

    renderTimerPage();

    expect(
      screen.getByRole("spinbutton", { name: "作業時間" }),
    ).not.toBeDisabled();
    expect(
      screen.getByText(
        "実行中のセッションの長さは変わりません。保存した時間は次のセッションから使われます。",
      ),
    ).toBeInTheDocument();
  });

  it.each([null, "idle"] as const)(
    "does not claim a session is running when status is %s",
    (status) => {
      useTimerObservationStore.setState({ status });
      renderTimerPage();

      expect(
        screen.queryByTestId("settings-duration-applies-next-session"),
      ).not.toBeInTheDocument();
    },
  );

  it("focuses the timer heading on mount", async () => {
    renderTimerPage();

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "タイマー設定" }),
      ).toHaveFocus();
    });
  });
});
