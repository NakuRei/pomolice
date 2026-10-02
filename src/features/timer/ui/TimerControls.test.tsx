import { useBackendObservationStore } from "@/backend/observationStore";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { resetAppStateForTest } from "@/test/resetAppState";
import { invoke } from "@tauri-apps/api/core";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimerControls } from "./TimerControls";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("TimerControls", () => {
  beforeEach(() => {
    resetAppStateForTest();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({}, { status: "working" }),
    });
    useBackendObservationStore.setState({ status: "idle" });
  });

  it("starts work using backend settings and replaces start with emergency stop", async () => {
    const user = userEvent.setup();
    render(<TimerControls />);

    expect(
      screen.queryByRole("button", { name: "緊急停止" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "タイマーを開始" }));

    expect(invoke).toHaveBeenCalledExactlyOnceWith("start_work");
    expect(
      await screen.findByRole("button", { name: "緊急停止" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "タイマーを開始" }),
    ).not.toBeInTheDocument();
  });

  it("opens the emergency stop confirmation while working", async () => {
    useBackendObservationStore.setState({ status: "working" });
    const user = userEvent.setup();
    render(<TimerControls />);

    await user.click(screen.getByRole("button", { name: "緊急停止" }));

    expect(
      screen.getByRole("dialog", { name: "緊急停止しますか？" }),
    ).toBeInTheDocument();
  });

  it("offers start again when the backend reports idle", () => {
    useBackendObservationStore.setState({ status: "working" });
    render(<TimerControls />);
    expect(
      screen.queryByRole("button", { name: "タイマーを開始" }),
    ).not.toBeInTheDocument();

    act(() => {
      useBackendObservationStore
        .getState()
        .observeSnapshot(createTimerSnapshotForTest());
    });

    expect(
      screen.getByRole("button", { name: "タイマーを開始" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "緊急停止" }),
    ).not.toBeInTheDocument();
  });
});
