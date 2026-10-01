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

import {
  createSettingsForTest,
  createWeeklyAutoRunConfigForTest,
} from "@/test/settingsFixture";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import type { Settings } from "@/types/settings";
import { SettingsTopPage } from "./SettingsTopPage";

function savedSettingsChanges(): Partial<SettingsInput>[] {
  return mockInvoke.mock.calls
    .filter(([command]) => command === "update_settings")
    .map(([, args]) => args?.changes as Partial<SettingsInput>);
}

/** Renders the page the way the settings screen does: from the store. */
function SettingsTopPageFromStore({
  openHomeScreen = (): void => undefined,
}: {
  openHomeScreen?: () => void;
}): React.ReactElement {
  const settings = useTimerObservationStore((state) => state.settings);
  return (
    <SettingsTopPage
      settings={settings}
      openHomeScreen={openHomeScreen}
      openTimerSettings={() => undefined}
      openAutoRunSettings={() => undefined}
      openNotificationSettings={() => undefined}
    />
  );
}

function renderWithSettings(settings: Settings | null): void {
  useTimerObservationStore.setState({ settings });
  render(<SettingsTopPageFromStore />);
}

describe("SettingsTopPage", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(answerSettingsSavesForTest);
    resetTimerStoreForTest();
  });

  it("renders timer and auto-run summaries on settings top", () => {
    renderWithSettings(
      createSettingsForTest({
        workDurationSecs: 30 * 60,
        shortBreakDurationSecs: 6 * 60,
        longBreakDurationSecs: 20 * 60,
        longBreakInterval: 5,
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
          mon: { enabled: true, startTime: "09:00", endTime: "18:00" },
          tue: { enabled: true, startTime: "09:00", endTime: "18:00" },
        }),
        postBreakTransitionPolicy: "start_next_work_automatically",
        notificationSoundMuted: true,
        notificationSoundVolumePercent: 35,
      }),
    );

    expect(
      screen.getByRole("button", { name: /^タイマー設定/u }),
    ).toHaveTextContent("作業30分 / 短休憩6分 / 長休憩20分 / 間隔5回");
    expect(
      screen.getByRole("button", { name: /^曜日別自動実行/u }),
    ).toHaveTextContent("月火 09:00-18:00");
    expect(
      screen.getByText("休憩後に自動で次の作業を開始する"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^通知/u })).toHaveTextContent(
      "ミュートON / 音量35%",
    );
  });

  it("shows durations that are not whole minutes with seconds", () => {
    renderWithSettings(createSettingsForTest({ shortBreakDurationSecs: 90 }));

    expect(
      screen.getByRole("button", { name: /^タイマー設定/u }),
    ).toHaveTextContent("短休憩1分30秒");
  });

  it("shows placeholders and disables every editor until the backend has sent settings", () => {
    renderWithSettings(null);

    expect(
      screen.getByRole("button", { name: /^タイマー設定/u }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: /^曜日別自動実行/u }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: /^通知/u })).toBeDisabled();
    expect(
      screen.getByTestId("settings-top-post-break-policy-toggle"),
    ).toBeDisabled();
    expect(screen.getAllByText("読み込み中").length).toBeGreaterThan(0);
  });

  it("focuses the settings top heading on mount", async () => {
    renderWithSettings(createSettingsForTest());

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "設定" })).toHaveFocus();
    });
  });

  it("renders the shared back button on settings top and calls home navigation", async () => {
    const user = userEvent.setup();
    const openHomeScreen = vi.fn();
    useTimerObservationStore.setState({ settings: createSettingsForTest() });
    render(<SettingsTopPageFromStore openHomeScreen={openHomeScreen} />);

    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(openHomeScreen).toHaveBeenCalledTimes(1);
  });

  it("shows compact auto-run summary when enabled weekdays have multiple windows", () => {
    renderWithSettings(
      createSettingsForTest({
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
          mon: { enabled: true, startTime: "09:00", endTime: "18:00" },
          tue: { enabled: true, startTime: "10:00", endTime: "19:00" },
        }),
      }),
    );

    expect(
      screen.getByRole("button", { name: /^曜日別自動実行/u }),
    ).toHaveTextContent("有効2日（詳細参照）");
  });

  it("saves the switched post-break transition policy and shows the saved value", async () => {
    const user = userEvent.setup();
    renderWithSettings(
      createSettingsForTest({
        postBreakTransitionPolicy: "require_manual_continue",
        notificationSoundVolumePercent: 35,
      }),
    );

    expect(screen.getByText("毎回確認する")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "自動継続をONにする" }),
    );

    expect(
      await screen.findByRole("button", { name: "毎回確認に戻す" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByText("休憩後に自動で次の作業を開始する"),
    ).toBeInTheDocument();
    expect(savedSettingsChanges()).toEqual([
      { post_break_transition_policy: "start_next_work_automatically" },
    ]);
  });
});
