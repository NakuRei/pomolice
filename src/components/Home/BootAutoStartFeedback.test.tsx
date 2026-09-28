import { TimerEffectsNotice } from "@/components/shared/TimerEffectsNotice";
import { useTimerEffectsStore } from "@/store/timerEffectsStore";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";
import type { TimerSnapshot } from "@/types/timerEffects";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { HomeScreen } from "./HomeScreen";
import type { BackendErrorResponse } from "@/errors/classifyBackendError";
import { useErrorStore } from "@/store/errorStore";
import { useTimerStore } from "@/store/timerStore";
import { TimerStatus } from "@/types/timer";
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
  message: "please wait",
};

describe("startup confirmation feedback", () => {
  beforeEach(() => {
    vi.mocked(invoke)
      .mockReset()
      .mockResolvedValue(createTimerSnapshotForTest());
    resetTimerStoreForTest();
    useErrorStore.getState().clearAllErrors();
    useTimerStore.setState({
      settings: createSettingsForTest(),
      isConnected: true,
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
      vi.mocked(invoke).mockReturnValueOnce(pendingCommand);
      renderHome();
      const user = userEvent.setup();
      await user.click(screen.getByRole("button", { name: "開始する" }));
      const effects = createTimerEffectsForTest({
        revision: 1,
        last_timer_save: "failed",
      });
      act(() => {
        useTimerStore.setState({ status: TimerStatus.WORKING });
        if (order === "close first")
          useTimerStore.getState().clearBootAutoStartOffer();
        useTimerEffectsStore.getState().observe(effects);
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
      expect(useTimerStore.getState().status).toBe(TimerStatus.WORKING);
      expect(invoke).toHaveBeenCalledTimes(1);
      vi.mocked(invoke).mockResolvedValueOnce(
        createTimerSnapshotForTest({
          effects: createTimerEffectsForTest({
            revision: 2,
            last_timer_save: "succeeded",
          }),
        }),
      );
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
      vi.mocked(invoke).mockRejectedValueOnce(rateLimited);
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
        useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
      ).toBeUndefined();
    },
  );

  it("keeps the error after expiry but removes retry and makes a captured retry harmless", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(rateLimited);
    renderHome();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "開始する" }));
    const retry =
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt?.retryFn;
    if (!retry) {
      throw new Error("Expected a retry while the offer is valid");
    }
    expect(screen.getByRole("button", { name: "再試行" })).toBeInTheDocument();

    act(() => {
      useTimerStore.getState().clearBootAutoStartOffer();
    });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("少々お待ちください");
    expect(
      screen.queryByRole("button", { name: "再試行" }),
    ).not.toBeInTheDocument();
    await act(async () => {
      await retry();
    });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent("少々お待ちください");
  });
});
