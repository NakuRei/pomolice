import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { AutoStartConfirmDialog } from "./AutoStartConfirmDialog";
import { useTimerStore } from "@/store/timerStore";

import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation(() =>
    Promise.resolve({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    }),
  ),
}));

const MONDAY_DAYTIME_WINDOW = {
  weekday_key: "mon",
  start_time: "09:00",
  end_time: "18:00",
  crosses_midnight: false,
} as const;

function setDialogVisible(): void {
  useTimerStore.setState({
    bootAutoStartOffer: createBootAutoStartOfferForTest(),
    activeWindowSummary: MONDAY_DAYTIME_WINDOW,
  });
}

function getAutoStartConfirmDialog(): HTMLElement {
  return screen.getByRole("dialog", { name: "作業を開始しますか？" });
}

describe("AutoStartConfirmDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTimerStore.setState({
      status: "idle",
      remainingSeconds: 25 * 60,
      sessionType: "work",
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
      isConnected: true,
      bootAutoStartOffer: null,
      activeWindowSummary: null,
    });
  });

  it("does not render when startup prompt is not required", () => {
    render(<AutoStartConfirmDialog />);
    expect(
      screen.queryByRole("dialog", { name: "作業を開始しますか？" }),
    ).not.toBeInTheDocument();
  });

  it("does not render before the startup offer is known", () => {
    useTimerStore.setState({ bootAutoStartOffer: undefined });
    render(<AutoStartConfirmDialog />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes when the backend closes the startup offer", () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    act(() => {
      useTimerStore.getState().clearBootAutoStartOffer();
    });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("stays open with the boot-time window after the live window ends", () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    act(() => {
      useTimerStore
        .getState()
        .handleActiveWindowChanged({ active_window_summary: null });
    });

    expect(getAutoStartConfirmDialog()).toBeInTheDocument();
    expect(
      screen.getByText(/現在は 月曜日 09:00 - 18:00/u),
    ).toBeInTheDocument();
  });

  it("does not render when only the live active window exists without a boot prompt", () => {
    useTimerStore.setState({
      bootAutoStartOffer: null,
      activeWindowSummary: MONDAY_DAYTIME_WINDOW,
    });

    render(<AutoStartConfirmDialog />);
    expect(
      screen.queryByRole("dialog", { name: "作業を開始しますか？" }),
    ).not.toBeInTheDocument();
  });

  it("renders current window summary when startup prompt is required", () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    const dialog = getAutoStartConfirmDialog();

    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(
      screen.getByText(/現在は 月曜日 09:00 - 18:00/u),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "開始すると終了時刻連動でセッション区切り停止されます。",
      ),
    ).toBeInTheDocument();
  });

  it("focuses the start button on dialog open", async () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    const startButton = screen.getByRole("button", { name: "開始する" });
    await waitFor(() => {
      expect(startButton).toHaveFocus();
    });
  });

  it("calls resolve_boot_auto_start_prompt(true) when start button is clicked", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByRole("button", { name: "開始する" }));

    expect(invokeMock).toHaveBeenCalledWith("resolve_boot_auto_start_prompt", {
      startNow: true,
    });
  });

  it("calls resolve_boot_auto_start_prompt(false) when later button is clicked", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByRole("button", { name: "あとで開始する" }));

    expect(invokeMock).toHaveBeenCalledWith("resolve_boot_auto_start_prompt", {
      startNow: false,
    });
  });

  it("hides after choosing later and stays hidden in the same app run", async () => {
    const user = userEvent.setup();
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByRole("button", { name: "あとで開始する" }));

    await waitFor(() => {
      expect(
        screen.queryByRole("dialog", { name: "作業を開始しますか？" }),
      ).not.toBeInTheDocument();
    });
  });

  it("becomes eligible again after store reinitialization (simulated app restart)", async () => {
    const user = userEvent.setup();
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByRole("button", { name: "あとで開始する" }));
    await waitFor(() => {
      expect(
        screen.queryByRole("dialog", { name: "作業を開始しますか？" }),
      ).not.toBeInTheDocument();
    });

    act(() => {
      useTimerStore.setState({
        bootAutoStartOffer: null,
        activeWindowSummary: null,
      });
      setDialogVisible();
    });

    await waitFor(() => {
      expect(
        screen.getByRole("dialog", { name: "作業を開始しますか？" }),
      ).toBeInTheDocument();
    });
  });

  it("treats Escape as choosing later", async () => {
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "resolve_boot_auto_start_prompt",
        {
          startNow: false,
        },
      );
    });
  });

  it("does not close or invoke action on backdrop click", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-dialog-overlay"));

    expect(invokeMock).not.toHaveBeenCalled();
    expect(getAutoStartConfirmDialog()).toBeInTheDocument();
  });
});
