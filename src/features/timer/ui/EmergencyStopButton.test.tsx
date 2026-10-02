import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "@/backend/observationStore";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { resetAppStateForTest } from "@/test/resetAppState";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmergencyStopButton } from "./EmergencyStopButton";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

async function openEmergencyStopDialog(
  user: ReturnType<typeof userEvent.setup>,
): Promise<HTMLElement> {
  await user.click(screen.getByRole("button", { name: "緊急停止" }));
  return screen.getByRole("dialog", { name: "緊急停止しますか？" });
}

describe("EmergencyStopButton", () => {
  beforeEach(() => {
    resetAppStateForTest();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    useBackendObservationStore.setState({ status: "working" });
  });

  it("does not offer emergency stop while idle", () => {
    useBackendObservationStore.setState({ status: "idle" });
    render(<EmergencyStopButton />);

    expect(
      screen.queryByRole("button", { name: "緊急停止" }),
    ).not.toBeInTheDocument();
  });

  it("opens a named confirmation dialog with the warning and initially focuses cancel", async () => {
    const user = userEvent.setup();
    render(<EmergencyStopButton />);

    const dialog = await openEmergencyStopDialog(user);

    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(
      within(dialog).getByText(/現在のポモドーロセッションは記録されません/u),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "停止する" }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      ).toHaveFocus();
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it.each(["cancel", "Escape"] as const)(
    "dismisses with %s, restores focus, and leaves the timer running",
    async (action) => {
      const user = userEvent.setup();
      render(<EmergencyStopButton />);
      const trigger = screen.getByRole("button", { name: "緊急停止" });
      const dialog = await openEmergencyStopDialog(user);

      if (action === "cancel") {
        await user.click(
          within(dialog).getByRole("button", { name: "キャンセル" }),
        );
      } else {
        await user.keyboard("{Escape}");
      }

      await waitFor(() => {
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        expect(trigger).toHaveFocus();
      });
      expect(invoke).not.toHaveBeenCalled();
      expect(useBackendObservationStore.getState().status).toBe("working");
    },
  );

  it("sends emergency stop on confirmation and adopts the backend's stopped state", async () => {
    useBackendObservationStore.setState({ completedPomodoros: 2 });
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest({}, { completed_pomodoros: 3 }),
    });
    const user = userEvent.setup();
    render(<EmergencyStopButton />);
    const dialog = await openEmergencyStopDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "停止する" }));

    expect(invoke).toHaveBeenCalledExactlyOnceWith("emergency_stop");
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(useBackendObservationStore.getState()).toMatchObject({
        status: "idle",
        completedPomodoros: 3,
      });
    });
  });

  it("keeps the dialog open and shows an error when confirmation fails", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    const user = userEvent.setup();
    render(<EmergencyStopButton />);
    const dialog = await openEmergencyStopDialog(user);

    await user.click(within(dialog).getByRole("button", { name: "停止する" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "少々お待ちください",
    );
    expect(dialog).toBeInTheDocument();
    expect(useBackendObservationStore.getState().status).toBe("working");
  });
});
