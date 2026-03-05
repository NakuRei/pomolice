import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { AutoStartConfirmDialog } from "../AutoStartConfirmDialog";
import { SessionType, TimerStatus, useTimerStore } from "../../store/timerStore";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("resolved_later"),
}));

function setDialogVisible() {
  useTimerStore.setState({
    shouldPromptStartOnBoot: true,
    activeWindowSummary: {
      weekday_key: "mon",
      start_time: "09:00",
      end_time: "18:00",
      crosses_midnight: false,
    },
  });
}

describe("AutoStartConfirmDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      isFullscreenBreak: false,
      requiresManualContinueDecision: false,
      isConnected: true,
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
    });
  });

  it("does not render when startup prompt is not required", () => {
    render(<AutoStartConfirmDialog />);
    expect(screen.queryByTestId("auto-start-confirm-dialog")).not.toBeInTheDocument();
  });

  it("does not render when prompt flag is true but active window summary is missing", () => {
    useTimerStore.setState({
      shouldPromptStartOnBoot: true,
      activeWindowSummary: null,
    });

    render(<AutoStartConfirmDialog />);
    expect(screen.queryByTestId("auto-start-confirm-dialog")).not.toBeInTheDocument();
  });

  it("does not render when active window summary exists but prompt flag is false", () => {
    useTimerStore.setState({
      shouldPromptStartOnBoot: false,
      activeWindowSummary: {
        weekday_key: "mon",
        start_time: "09:00",
        end_time: "18:00",
        crosses_midnight: false,
      },
    });

    render(<AutoStartConfirmDialog />);
    expect(screen.queryByTestId("auto-start-confirm-dialog")).not.toBeInTheDocument();
  });

  it("renders current window summary when startup prompt is required", () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    expect(screen.getByTestId("auto-start-confirm-dialog")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toHaveAttribute("aria-modal", "true");
    expect(screen.getByText(/現在は 月曜日 09:00 - 18:00/)).toBeInTheDocument();
    expect(
      screen.getByText("開始すると終了時刻連動でセッション区切り停止されます。")
    ).toBeInTheDocument();
  });

  it("focuses the start button on dialog open", async () => {
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    const startButton = screen.getByTestId("auto-start-confirm-start");
    await waitFor(() => {
      expect(startButton).toHaveFocus();
    });
  });

  it("calls resolve_boot_auto_start_prompt(true) when start button is clicked", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-start"));

    expect(invokeMock).toHaveBeenCalledWith("resolve_boot_auto_start_prompt", {
      startNow: true,
    });
  });

  it("calls resolve_boot_auto_start_prompt(false) when later button is clicked", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-later"));

    expect(invokeMock).toHaveBeenCalledWith("resolve_boot_auto_start_prompt", {
      startNow: false,
    });
  });

  it("hides after choosing later and stays hidden in the same app run", async () => {
    const user = userEvent.setup();
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-later"));

    await waitFor(() => {
      expect(screen.queryByTestId("auto-start-confirm-dialog")).not.toBeInTheDocument();
    });
  });

  it("becomes eligible again after store reinitialization (simulated app restart)", async () => {
    const user = userEvent.setup();
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-later"));
    await waitFor(() => {
      expect(screen.queryByTestId("auto-start-confirm-dialog")).not.toBeInTheDocument();
    });

    await act(async () => {
      useTimerStore.setState({
        shouldPromptStartOnBoot: false,
        activeWindowSummary: null,
      });
      setDialogVisible();
    });

    await waitFor(() => {
      expect(screen.getByTestId("auto-start-confirm-dialog")).toBeInTheDocument();
    });
  });

  it("treats Escape as choosing later", async () => {
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("resolve_boot_auto_start_prompt", {
        startNow: false,
      });
    });
  });

  it("does not close or invoke action on backdrop click", async () => {
    const user = userEvent.setup();
    const invokeMock = vi.mocked(invoke);
    setDialogVisible();
    render(<AutoStartConfirmDialog />);

    await user.click(screen.getByTestId("auto-start-confirm-dialog-overlay"));

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId("auto-start-confirm-dialog")).toBeInTheDocument();
  });
});
