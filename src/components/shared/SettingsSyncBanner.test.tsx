import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  type SettingsApplicationStatus,
  useSettingsStore,
} from "@/store/settingsStore";
import { useErrorStore } from "@/store/errorStore";
import {
  SessionType,
  TimerStatus,
  useTimerStore,
  type FullStatePayload,
} from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { SettingsSyncBanner } from "./SettingsSyncBanner";

const RETRY_BUTTON_NAME = "設定の同期を再試行";

function setApplicationStatuses(
  durationStatus: SettingsApplicationStatus,
  automationStatus: SettingsApplicationStatus,
): void {
  useSettingsStore.setState({
    durationSettingsApplicationStatus: durationStatus,
    automationSettingsApplicationStatus: automationStatus,
  });
}

function createConnectedFullState(): FullStatePayload {
  return {
    status: TimerStatus.IDLE,
    remaining_seconds: 1500,
    session_type: SessionType.WORK,
    current_cycle: 1,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    is_fullscreen_break: false,
    post_break_decision_pending: false,
    settings: {
      work_duration_secs: 1500,
      short_break_duration_secs: 300,
      long_break_duration_secs: 900,
      long_break_interval: 4,
      post_break_transition_policy: "require_manual_continue",
    },
    should_prompt_start_on_boot: false,
    active_window_summary: null,
  };
}

describe("SettingsSyncBanner", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useErrorStore.getState().clearAllErrors();
    useSettingsStore.getState().resetToDefaults();
    setApplicationStatuses("confirmed", "confirmed");
    useTimerStore.setState({
      isConnected: true,
      status: TimerStatus.IDLE,
      isSettingsSynchronizationInFlight: false,
    });
  });

  it("renders nothing after both setting groups are confirmed", () => {
    render(<SettingsSyncBanner />);

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("announces a pending candidate without reporting it as applied", () => {
    setApplicationStatuses("pending", "confirmed");

    render(<SettingsSyncBanner />);

    const banner = screen.getByRole("status");
    expect(banner).toHaveAttribute("aria-live", "polite");
    expect(banner).toHaveTextContent(/適用はまだ確認されていません/u);
  });

  it("exposes an unconfirmed candidate as an alert", () => {
    render(
      <>
        <button type="button">既存の操作</button>
        <SettingsSyncBanner />
      </>,
    );
    const existingControl = screen.getByRole("button", {
      name: "既存の操作",
    });
    existingControl.focus();

    act(() => {
      setApplicationStatuses("confirmed", "unconfirmed");
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      /適用を確認できませんでした/u,
    );
    expect(existingControl).toHaveFocus();
  });

  it("offers Retry for a connected idle duration candidate", () => {
    setApplicationStatuses("pending", "confirmed");

    render(<SettingsSyncBanner />);

    expect(
      screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toBeInTheDocument();
  });

  it("offers Retry independently of a generic non-retryable error", () => {
    useErrorStore.getState().reportError({
      id: "settings-sync-generic",
      message: "設定を同期できませんでした。",
      technicalDetail: "invalid_command_input",
      severity: "recoverable",
      origin: "settings_sync",
      canRetry: false,
      retryFn: null,
    });
    setApplicationStatuses("pending", "confirmed");

    render(<SettingsSyncBanner />);

    expect(
      screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toBeInTheDocument();
  });

  it.each([TimerStatus.IDLE, TimerStatus.WORKING])(
    "offers Retry for automation while the timer is %s",
    (status) => {
      setApplicationStatuses("confirmed", "pending");
      useTimerStore.setState({ status });

      render(<SettingsSyncBanner />);

      expect(
        screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
      ).toBeInTheDocument();
    },
  );

  it("keeps automation synchronization informational during an enforced break", () => {
    setApplicationStatuses("confirmed", "unconfirmed");
    useTimerStore.setState({ status: TimerStatus.ON_BREAK });

    render(<SettingsSyncBanner />);

    expect(screen.getByRole("alert")).toHaveTextContent(/休憩終了後/u);
    expect(
      screen.queryByRole("button", { name: RETRY_BUTTON_NAME }),
    ).not.toBeInTheDocument();
  });

  it("explains both wait conditions for mixed unresolved settings during a break", () => {
    setApplicationStatuses("pending", "unconfirmed");
    useTimerStore.setState({ status: TimerStatus.ON_BREAK });

    render(<SettingsSyncBanner />);

    const banner = screen.getByRole("alert");
    expect(banner).toHaveTextContent(/休憩終了後/u);
    expect(banner).toHaveTextContent(/アイドル状態/u);
    expect(
      screen.queryByRole("button", { name: RETRY_BUTTON_NAME }),
    ).not.toBeInTheDocument();
  });

  it.each([TimerStatus.WORKING, TimerStatus.ON_BREAK])(
    "explains that duration waits for idle while the timer is %s",
    (status) => {
      setApplicationStatuses("pending", "confirmed");
      useTimerStore.setState({ status });

      render(<SettingsSyncBanner />);

      expect(
        screen.queryByRole("button", { name: RETRY_BUTTON_NAME }),
      ).not.toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent(/アイドル状態/u);
    },
  );

  it("explains that unresolved settings wait for a backend connection", () => {
    setApplicationStatuses("unconfirmed", "confirmed");
    useTimerStore.setState({ isConnected: false });

    render(<SettingsSyncBanner />);

    expect(
      screen.queryByRole("button", { name: RETRY_BUTTON_NAME }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(/再接続/u);

    act(() => {
      useTimerStore.getState().handleFullState(createConnectedFullState());
    });

    expect(screen.getByRole("alert")).not.toHaveTextContent(/再接続/u);
    expect(
      screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toBeInTheDocument();
  });

  it("explains that unresolved settings wait for an in-flight pass", () => {
    setApplicationStatuses("pending", "confirmed");
    useTimerStore.setState({ isSettingsSynchronizationInFlight: true });

    render(<SettingsSyncBanner />);

    expect(
      screen.queryByRole("button", { name: RETRY_BUTTON_NAME }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(/同期の完了/u);

    act(() => {
      useTimerStore.setState({ isSettingsSynchronizationInFlight: false });
    });

    expect(screen.getByRole("status")).not.toHaveTextContent(/同期の完了/u);
    expect(
      screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toBeInTheDocument();
  });

  it("requests one pass for two eligible groups", async () => {
    const user = userEvent.setup();
    const syncSettings = vi.fn<() => Promise<void>>(() => Promise.resolve());
    setApplicationStatuses("pending", "pending");
    useTimerStore.setState({ syncSettings });

    render(<SettingsSyncBanner />);

    const retryButton = screen.getByRole("button", {
      name: RETRY_BUTTON_NAME,
    });
    expect(
      screen.getAllByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toHaveLength(1);

    await user.click(retryButton);

    expect(syncSettings).toHaveBeenCalledTimes(1);
  });

  it("does not expose a dismiss control", () => {
    setApplicationStatuses("pending", "confirmed");

    render(<SettingsSyncBanner />);

    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(
      screen.getByRole("button", { name: RETRY_BUTTON_NAME }),
    ).toBeInTheDocument();
  });

  it("disappears only after both groups become confirmed", () => {
    setApplicationStatuses("pending", "unconfirmed");

    render(<SettingsSyncBanner />);
    expect(screen.getByRole("alert")).toBeInTheDocument();

    act(() => {
      setApplicationStatuses("confirmed", "unconfirmed");
    });
    expect(screen.getByRole("alert")).toBeInTheDocument();

    act(() => {
      setApplicationStatuses("confirmed", "confirmed");
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("absorbs a rejected Retry and remains visible", async () => {
    const user = userEvent.setup();
    const syncSettings = vi.fn<() => Promise<void>>(() =>
      Promise.reject(new Error("settings write failed")),
    );
    setApplicationStatuses("unconfirmed", "confirmed");
    useTimerStore.setState({ syncSettings });

    render(<SettingsSyncBanner />);

    await user.click(screen.getByRole("button", { name: RETRY_BUTTON_NAME }));

    await waitFor(() => {
      expect(syncSettings).toHaveBeenCalledTimes(1);
    });
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });
});
