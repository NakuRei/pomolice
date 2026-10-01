import { useTimerObservationStore } from "@/store/timerObservationStore";
import { acceptedCommand } from "@/test/commandResponseFixture";
import type { TimerSnapshot, BackendErrorResponse } from "@/ipc/generated";
import { TimerEffectsNotice } from "@/components/shared/TimerEffectsNotice";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { HomeScreen } from "./HomeScreen";

import { useTimerOperationStore } from "@/store/timerOperationStore";
import { retryRejectedOperation } from "@/timer/retryOperation";

import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function renderHome(): void {
  render(
    <>
      <TimerEffectsNotice />
      <HomeScreen
        notificationSoundMuted={false}
        isAutoRunWindowActive={true}
        isAutoContinueEnabled={false}
        requiresManualContinue={false}
        onToggleNotificationSoundMuted={() => undefined}
        onOpenSettingsTopScreen={() => undefined}
      />
    </>,
  );
}

const rateLimited: BackendErrorResponse = {
  code: "rate_limited",
};

describe("startup confirmation feedback", () => {
  beforeEach(() => {
    vi.mocked(invoke)
      .mockReset()
      .mockResolvedValue(acceptedCommand(createTimerSnapshotForTest()));
    resetTimerStoreForTest();
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
    useTimerObservationStore.setState({
      settings: createSettingsForTest(),

      bootAutoStartOffer: createBootAutoStartOfferForTest(),
    });
  });

  it.each(["close first", "failure first"])(
    "keeps a save failure visible when the backend sends %s",
    async (order) => {
      let resolveCommand: (result: TimerSnapshot) => void = () => {
        throw new Error("Expected a pending command");
      };
      const pendingCommand = new Promise<TimerSnapshot>((resolve) => {
        resolveCommand = resolve;
      });
      vi.mocked(invoke).mockReturnValueOnce(
        pendingCommand.then(acceptedCommand),
      );
      renderHome();
      const user = userEvent.setup();
      await user.click(screen.getByRole("button", { name: "開始する" }));
      const effects = createTimerEffectsForTest({
        revision: "1",
        last_timer_save: "failed",
      });
      act(() => {
        useTimerObservationStore.setState({ status: "working" });
        if (order === "close first")
          useTimerObservationStore.getState().clearBootAutoStartOffer();
        useTimerObservationStore.getState().observeEffects(effects);
      });
      if (order === "failure first") {
        const dialog = screen.getByRole("dialog", {
          name: "作業を開始しますか？",
        });
        expect(within(dialog).getByRole("alert")).toHaveTextContent(
          "タイマーの状態の保存に失敗しました",
        );
      }
      await act(async () => {
        resolveCommand(createTimerSnapshotForTest({ effects }));
        await pendingCommand;
      });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(screen.getByRole("alert")).toHaveTextContent(
        "タイマーの状態の保存に失敗しました",
      );
      expect(useTimerObservationStore.getState().status).toBe("working");
      expect(invoke).toHaveBeenCalledTimes(1);
      vi.mocked(invoke).mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest({
          effects: createTimerEffectsForTest({
            revision: "2",
            last_timer_save: "succeeded",
          }),
        }),
      });
      await user.click(screen.getByRole("button", { name: "再試行" }));
      expect(invoke).toHaveBeenLastCalledWith("retry_failed_timer_effects");
      expect(invoke).toHaveBeenCalledTimes(2);
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    },
  );

  it.each([
    { label: "開始する", startNow: true },
    { label: "あとで開始する", startNow: false },
  ])(
    "retries the original $label answer while the offer is valid",
    async ({ label, startNow }) => {
      vi.mocked(invoke).mockResolvedValueOnce({
        status: "rejected",
        error: rateLimited,
      });
      renderHome();
      const user = userEvent.setup();

      await user.click(screen.getByRole("button", { name: label }));

      const dialog = screen.getByRole("dialog", {
        name: "作業を開始しますか？",
      });
      expect(within(dialog).getByRole("alert")).toHaveTextContent(
        "少々お待ちください",
      );
      expect(screen.getAllByRole("alert", { hidden: true })).toHaveLength(1);
      await user.click(within(dialog).getByRole("button", { name: "再試行" }));

      await waitFor(() => {
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      });
      expect(invoke).toHaveBeenCalledTimes(2);
      expect(invoke).toHaveBeenLastCalledWith(
        "resolve_boot_auto_start_prompt",
        { startNow },
      );
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      expect(
        useTimerOperationStore.getState().operations
          .resolve_boot_auto_start_prompt,
      ).toMatchObject({ outcome: { status: "accepted" } });
    },
  );

  it("keeps the error after expiry but removes retry and makes a captured retry harmless", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: rateLimited,
    });
    renderHome();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "開始する" }));
    const retry =
      useTimerOperationStore.getState().operations
        .resolve_boot_auto_start_prompt;
    if (!retry) {
      throw new Error("Expected a retry while the offer is valid");
    }
    expect(screen.getByRole("button", { name: "再試行" })).toBeInTheDocument();

    act(() => {
      useTimerObservationStore.getState().clearBootAutoStartOffer();
    });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("少々お待ちください");
    expect(
      screen.queryByRole("button", { name: "再試行" }),
    ).not.toBeInTheDocument();
    await act(async () => {
      await retryRejectedOperation(retry);
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent("少々お待ちください");
  });
});
