import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { OperationNotice } from "./OperationNotice";
import { useOperationStore } from "@/backend/operationStore";
import { startWork, emergencyStop } from "@/features/timer/commands";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
describe("OperationNotice", () => {
  beforeEach(() => {
    resetAppStateForTest();
    vi.mocked(invoke).mockReset();
  });
  afterEach(() => {
    vi.useRealTimers();
  });
  it("renders nothing without a failed operation", () => {
    render(<OperationNotice operation="start_work" />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("retries a rejected operation and clears its notice on acceptance", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    await startWork();
    render(<OperationNotice operation="start_work" />);
    expect(screen.getByRole("alert")).toHaveTextContent("少々お待ちください");
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "再試行" }));
      await Promise.resolve();
    });
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("dismisses the notice without erasing an unknown result", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke).mockRejectedValue({ code: "rate_limited" });
    await startWork();
    render(<OperationNotice operation="start_work" />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "操作の結果を確認できませんでした",
    );
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      useOperationStore.getState().operations.start_work?.outcome.status,
    ).toBe("unavailable");
  });
  it("preserves critical emergency feedback until another operation or dismissal", async () => {
    vi.useFakeTimers();
    vi.mocked(invoke).mockResolvedValue({
      status: "rejected",
      error: { code: "invalid_state_transition" },
    });
    await emergencyStop();
    render(<OperationNotice operation="emergency_stop" />);
    act(() => {
      vi.advanceTimersByTime(10000);
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "現在の状態では実行できません",
    );
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
  it("allows the caller to suppress a no longer applicable retry", async () => {
    vi.mocked(invoke).mockResolvedValue({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    await startWork();
    render(<OperationNotice operation="start_work" onRetry={null} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
