import type {
  TimerSnapshot,
  TimerEffectsState,
  FullStatePayload,
} from "@/ipc/generated";
import {
  useTimerEffectsStore,
  selectBreakLockObservation,
} from "./timerEffectsStore";

import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

import { useTimerStore } from "./timerStore";

import { resetTimerStoreForTest } from "../test/resetTimerStore";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import { useErrorStore } from "./errorStore";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

function createFullStatePayload(
  overrides: Partial<FullStatePayload> = {},
  effects?: TimerEffectsState,
): TimerSnapshot {
  const snapshot = createTimerSnapshotForTest();
  return {
    timer: { ...snapshot.timer, ...overrides },
    effects: effects ?? snapshot.effects,
  };
}

describe("timerStore", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    resetTimerStoreForTest();
  });

  describe("Initial State", () => {
    it("should have correct initial status", () => {
      const state = useTimerStore.getState();
      expect(state.status).toBe("idle");
    });

    it("should have correct initial remaining seconds (25 minutes)", () => {
      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(25 * 60);
    });

    it("should have correct initial session type", () => {
      const state = useTimerStore.getState();
      expect(state.sessionType).toBe("work");
    });

    it("should have initial completed pomodoros as 0", () => {
      const state = useTimerStore.getState();
      expect(state.completedPomodoros).toBe(0);
    });

    it("should have initial current cycle as 0", () => {
      const state = useTimerStore.getState();
      expect(state.currentCycle).toBe(0);
    });

    it("should not show continue dialog initially", () => {
      const state = useTimerStore.getState();
      expect(state.requiresManualContinueDecision).toBe(false);
    });
  });

  describe("Constants", () => {
    it("should export TimerStatus constants", () => {
      expect("idle").toBe("idle");
      expect("working").toBe("working");
      expect("on_break").toBe("on_break");
    });

    it("should export SessionType constants", () => {
      expect("work").toBe("work");
      expect("short_break").toBe("short_break");
      expect("long_break").toBe("long_break");
    });
  });

  describe("Event Handlers", () => {
    describe("handleTimerTick()", () => {
      it("should update state from tick payload", () => {
        const { handleTimerTick } = useTimerStore.getState();

        handleTimerTick({
          status: "working",
          remaining_seconds: 1234,
          session_duration_secs: 1500,
          session_type: "work",
          current_cycle: 2,
          completed_pomodoros: 1,
          break_interaction_count: 0,
          away_during_break: false,
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe("working");
        expect(state.remainingSeconds).toBe(1234);
        expect(state.sessionDurationSeconds).toBe(1500);
        expect(state.sessionType).toBe("work");
        expect(state.currentCycle).toBe(2);
        expect(state.completedPomodoros).toBe(1);
      });

      it("should update break state", () => {
        const { handleTimerTick } = useTimerStore.getState();

        handleTimerTick({
          status: "on_break",
          remaining_seconds: 300,
          session_duration_secs: 300,
          session_type: "short_break",
          current_cycle: 1,
          completed_pomodoros: 1,
          break_interaction_count: 3,
          away_during_break: false,
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe("on_break");
        expect(state.sessionType).toBe("short_break");
        expect(state.breakInteractionCount).toBe(3);
      });

      it("preserves the reported timer phase independently of a confirmed lock", () => {
        useTimerEffectsStore.getState().observe({
          revision: "1",
          last_timer_save: null,
          break_lock: {
            target: "locked",
            last_attempt: { action: "enter", status: "succeeded" },
          },
        });
        useTimerStore.setState({
          status: "working",
          isConnected: true,
        });

        useTimerStore.getState().handleTimerTick({
          status: "idle",
          remaining_seconds: 0,
          session_duration_secs: 1500,
          session_type: "work",
          current_cycle: 1,
          completed_pomodoros: 1,
          break_interaction_count: 0,
          away_during_break: false,
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe("idle");
        expect(
          selectBreakLockObservation(useTimerEffectsStore.getState()),
        ).toBe("locked");
      });
    });

    describe("setAwayDuringBreak()", () => {
      it("reports a backend rejection instead of swallowing it", async () => {
        mockInvoke.mockResolvedValueOnce({
          status: "rejected",
          error: {
            code: "invalid_state_transition",
            message: "invalid state",
          },
        });

        await expect(
          useTimerStore.getState().setAwayDuringBreak(true),
        ).rejects.toBeDefined();
        expect(
          useErrorStore.getState().errors.set_away_during_break,
        ).toBeDefined();
      });
    });

    describe("handleSessionCompleted()", () => {
      it("should show continue dialog when break completes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();

        handleSessionCompleted({
          transition_reason: "break_waiting_decision",
          completed_session_type: "short_break",
          next_status: "idle",
          next_session_type: "work",
          requires_manual_continue_decision: true,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(true);
      });

      it("should not show continue dialog when work completes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();

        handleSessionCompleted({
          transition_reason: "work_to_break",
          completed_session_type: "work",
          next_status: "on_break",
          next_session_type: "short_break",
          requires_manual_continue_decision: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when break auto-starts next work", () => {
        const { handleSessionCompleted } = useTimerStore.getState();
        useTimerStore.setState({ requiresManualContinueDecision: true });

        handleSessionCompleted({
          transition_reason: "break_auto_started",
          completed_session_type: "short_break",
          next_status: "working",
          next_session_type: "work",
          requires_manual_continue_decision: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when scheduled stop executes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();
        useTimerStore.setState({ requiresManualContinueDecision: true });

        handleSessionCompleted({
          transition_reason: "scheduled_stop_executed",
          completed_session_type: "short_break",
          next_status: "idle",
          next_session_type: "work",
          requires_manual_continue_decision: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });
    });

    describe("handleFullState()", () => {
      it("should update all state from full payload", () => {
        const settings = createSettingsForTest({ workDurationSecs: 50 * 60 });

        useTimerStore.getState().handleFullState(
          createFullStatePayload({
            status: "working",
            remaining_seconds: 1200,
            session_duration_secs: 1500,
            current_cycle: 3,
            completed_pomodoros: 2,
            settings: settingsPayloadForTest(settings),
          }),
        );

        const state = useTimerStore.getState();
        expect(state.status).toBe("working");
        expect(state.remainingSeconds).toBe(1200);
        expect(state.sessionDurationSeconds).toBe(1500);
        expect(state.currentCycle).toBe(3);
        expect(state.completedPomodoros).toBe(2);
        expect(state.settings).toEqual(settings);
        expect(state.settingsFileStatus).toBe("ok");
        expect(state.isConnected).toBe(true);
      });

      it("exposes that the settings file could not be used", () => {
        useTimerStore
          .getState()
          .handleFullState(
            createFullStatePayload({ settings_file_status: "unusable" }),
          );

        expect(useTimerStore.getState().settingsFileStatus).toBe("unusable");
      });

      it("keeps timer phase separate from the lock in a full snapshot", () => {
        useTimerStore.getState().handleFullState(
          createFullStatePayload(
            {},
            {
              revision: "1",
              last_timer_save: null,
              break_lock: {
                target: "locked",
                last_attempt: { action: "enter", status: "succeeded" },
              },
            },
          ),
        );
        expect(useTimerStore.getState().status).toBe("idle");
        expect(
          selectBreakLockObservation(useTimerEffectsStore.getState()),
        ).toBe("locked");
      });

      it("should keep startup prompt state from backend full payload", () => {
        const { handleFullState } = useTimerStore.getState();

        handleFullState(
          createFullStatePayload({
            current_cycle: 1,
            boot_auto_start_offer: createBootAutoStartOfferForTest(),
            active_window_summary: {
              weekday_key: "mon",
              start_time: "09:00",
              end_time: "18:00",
              crosses_midnight: false,
            },
          }),
        );

        const state = useTimerStore.getState();
        expect(state.bootAutoStartOffer).toEqual(
          createBootAutoStartOfferForTest(),
        );
        expect(state.activeWindowSummary).toEqual({
          weekday_key: "mon",
          start_time: "09:00",
          end_time: "18:00",
          crosses_midnight: false,
        });
      });
    });

    describe("setConnected()", () => {
      it("should update connection status", () => {
        const { setConnected } = useTimerStore.getState();

        expect(useTimerStore.getState().isConnected).toBe(false);

        setConnected(true);
        expect(useTimerStore.getState().isConnected).toBe(true);

        setConnected(false);
        expect(useTimerStore.getState().isConnected).toBe(false);
      });
    });

    describe("Info Message", () => {
      afterEach(() => {
        vi.useRealTimers();
      });

      it("should show stop-pending info message", () => {
        const { showAutoRunStopPendingInfo } = useTimerStore.getState();

        showAutoRunStopPendingInfo();

        const state = useTimerStore.getState();
        expect(state.infoMessage).not.toBeNull();
        expect(state.infoMessage?.id).toBe("auto_run_stop_pending");
        expect(state.infoMessage?.message).toBe(
          "現在セッション終了後に停止予定",
        );
        expect(state.infoMessage?.ttlMs).toBe(5000);
      });

      it("should suppress duplicate stop-pending info within ttl", () => {
        vi.useFakeTimers({ now: 1000 });
        const { showAutoRunStopPendingInfo, clearInfoMessage } =
          useTimerStore.getState();

        showAutoRunStopPendingInfo();
        expect(useTimerStore.getState().infoMessage?.id).toBe(
          "auto_run_stop_pending",
        );
        clearInfoMessage();

        vi.setSystemTime(5999);
        showAutoRunStopPendingInfo();
        expect(useTimerStore.getState().infoMessage).toBeNull();

        vi.setSystemTime(6000);
        showAutoRunStopPendingInfo();
        expect(useTimerStore.getState().infoMessage?.id).toBe(
          "auto_run_stop_pending",
        );
      });

      it("should show auto-run stopped info message", () => {
        const { showAutoRunStoppedInfo } = useTimerStore.getState();

        showAutoRunStoppedInfo();

        const state = useTimerStore.getState();
        expect(state.infoMessage).not.toBeNull();
        expect(state.infoMessage?.id).toBe("auto_run_stopped");
      });

      it("should clear info message only when expected id matches", () => {
        const { showAutoRunStoppedInfo, clearInfoMessage } =
          useTimerStore.getState();

        showAutoRunStoppedInfo();
        clearInfoMessage("auto_run_stop_pending");
        expect(useTimerStore.getState().infoMessage?.id).toBe(
          "auto_run_stopped",
        );

        clearInfoMessage("auto_run_stopped");
        expect(useTimerStore.getState().infoMessage).toBeNull();
      });
    });
  });
});
