import type { SettingsInput } from "@/ipc/generated";
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

vi.mock("@/services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(() =>
    Promise.resolve({ status: "played" }),
  ),
}));

import { createSettingsForTest } from "@/test/settingsFixture";
import { resetAppStateForTest } from "@/test/resetAppState";
import type { Settings } from "../../../backend/settings";
import { playNotificationSoundPreview } from "@/services/notifications";
import { SettingsNotificationPage } from "./SettingsNotificationPage";

function savedSettingsChanges(): Partial<SettingsInput>[] {
  return mockInvoke.mock.calls
    .filter(([command]) => command === "update_settings")
    .map(([, args]) => args?.changes as Partial<SettingsInput>);
}

function renderNotificationPage(
  settings: Settings = createSettingsForTest(),
  openSettingsTopScreen = vi.fn(),
): ReturnType<typeof vi.fn> {
  render(
    <SettingsNotificationPage
      settings={settings}
      openSettingsTopScreen={openSettingsTopScreen}
    />,
  );
  return openSettingsTopScreen;
}

describe("SettingsNotificationPage", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(answerSettingsSavesForTest);
    resetAppStateForTest();
    vi.mocked(playNotificationSoundPreview).mockResolvedValue({
      status: "played",
    });
  });

  it("saves only the notification settings", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderNotificationPage(
      createSettingsForTest({ workDurationSecs: 27 * 60 }),
    );

    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    fireEvent.change(volumeSlider, { target: { value: "35" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
    });
    expect(savedSettingsChanges()).toEqual([
      {
        notification_sound_volume_percent: 35,
        notification_sound_muted: false,
      },
    ]);
  });

  it("shows the backend's volume range error and stays on the page", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({
      status: "rejected",
      error: {
        code: "invalid_settings",
        message: "Invalid settings",
        issues: [
          {
            code: "out_of_range",
            field: "notification_sound_volume_percent",
            min: 0,
            max: 100,
          },
        ],
      },
    });
    const openSettingsTopScreen = renderNotificationPage();

    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(
      await screen.findByText("0〜100%の範囲で入力してください。"),
    ).toBeInTheDocument();
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("clears the volume error once the volume is changed", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({
      status: "rejected",
      error: {
        code: "invalid_settings",
        message: "Invalid settings",
        issues: [
          {
            code: "out_of_range",
            field: "notification_sound_volume_percent",
            min: 0,
            max: 100,
          },
        ],
      },
    });
    renderNotificationPage();
    await user.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByText("0〜100%の範囲で入力してください。");

    fireEvent.change(screen.getByRole("slider", { name: "通知音量" }), {
      target: { value: "40" },
    });

    expect(
      screen.queryByText("0〜100%の範囲で入力してください。"),
    ).not.toBeInTheDocument();
  });

  it("discards notification draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = renderNotificationPage();

    await user.click(screen.getByRole("button", { name: "通知音" }));
    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    await user.click(screen.getByRole("button", { name: "通知音" }));
    fireEvent.change(volumeSlider, { target: { value: "80" } });
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(savedSettingsChanges()).toEqual([]);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("disables volume and preview button when notification is muted", async () => {
    const user = userEvent.setup();

    renderNotificationPage();

    await user.click(screen.getByRole("button", { name: "通知音" }));

    expect(screen.getByRole("slider", { name: "通知音量" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    ).toBeDisabled();
    expect(
      screen.getByText("通知音がミュート中のため再生しませんでした。"),
    ).toBeInTheDocument();
  });

  it("initializes notification preview ui state on each mount", async () => {
    const user = userEvent.setup();

    const firstRender = render(
      <SettingsNotificationPage
        settings={createSettingsForTest()}
        openSettingsTopScreen={() => undefined}
      />,
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "テストする通知音の種類" }),
      "long_break",
    );
    await user.click(screen.getByRole("button", { name: "通知音" }));
    firstRender.unmount();

    renderNotificationPage();

    const previewTypeSelect = screen.getByRole("combobox", {
      name: "テストする通知音の種類",
    });
    expect(previewTypeSelect).toHaveValue("work");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveAttribute("aria-live", "polite");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveTextContent("");
  });

  it("shows unavailable message when preview sound file is not available", async () => {
    const user = userEvent.setup();
    vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({
      status: "unavailable",
    });

    renderNotificationPage();

    await user.click(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    );

    expect(playNotificationSoundPreview).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText(
        "通知音ファイルを読み込めなかったため再生できませんでした。",
      ),
    ).toBeInTheDocument();
  });

  it("focuses the notification heading on mount", async () => {
    renderNotificationPage();

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "通知設定" })).toHaveFocus();
    });
  });
});
