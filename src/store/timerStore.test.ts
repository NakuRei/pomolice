import {
  useTimerObservationStore,
  selectBreakLockObservation,
} from "@/store/timerObservationStore";
import { setAwayDuringBreak } from "@/timer/commands";
import { useTimerNoticeStore } from "@/store/timerNoticeStore";
import { useTimerSyncStore } from "@/store/timerSyncStore";
import type {
  TimerSnapshot,
  TimerEffectsState,
  FullStatePayload,
} from "@/ipc/generated";

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

import { resetTimerStoreForTest } from "../test/resetTimerStore";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import { useTimerOperationStore } from "@/operations/operationStore";
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

describe("@/store/timerObservationStore", () => {
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
      const state = useTimerObservationStore.getState();
      expect(state.status).toBeNull();
    });

    it("leaves remaining seconds unknown until observed", () => {
      const state = useTimerObservationStore.getState();
      expect(state.remainingSeconds).toBeNull();
    });

    it("should have correct initial session type", () => {
      const state = useTimerObservationStore.getState();
      expect(state.sessionType).toBeNull();
    });

    it("leaves completed pomodoros unknown until observed", () => {
      const state = useTimerObservationStore.getState();
      expect(state.completedPomodoros).toBeNull();
    });

    it("leaves current cycle unknown until observed", () => {
      const state = useTimerObservationStore.getState();
      expect(state.currentCycle).toBeNull();
    });

    it("should not show continue dialog initially", () => {
      const state = useTimerObservationStore.getState();
      expect(state.requiresManualContinueDecision).toBeNull();
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
    describe("observeTimerTick()", () => {
      it("should update state from tick payload", () => {
        const { observeTimerTick } = useTimerObservationStore.getState();

        observeTimerTick({
          status: "working",
          remaining_seconds: 1234,
          session_duration_secs: 1500,
          session_type: "work",
          current_cycle: 2,
          completed_pomodoros: 1,
          break_interaction_count: 0,
          away_during_break: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.status).toBe("working");
        expect(state.remainingSeconds).toBe(1234);
        expect(state.sessionDurationSeconds).toBe(1500);
        expect(state.sessionType).toBe("work");
        expect(state.currentCycle).toBe(2);
        expect(state.completedPomodoros).toBe(1);
      });

      it("should update break state", () => {
        const { observeTimerTick } = useTimerObservationStore.getState();

        observeTimerTick({
          status: "on_break",
          remaining_seconds: 300,
          session_duration_secs: 300,
          session_type: "short_break",
          current_cycle: 1,
          completed_pomodoros: 1,
          break_interaction_count: 3,
          away_during_break: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.status).toBe("on_break");
        expect(state.sessionType).toBe("short_break");
        expect(state.breakInteractionCount).toBe(3);
      });

      it("preserves the reported timer phase independently of a confirmed lock", () => {
        useTimerObservationStore.getState().observeEffects({
          revision: "1",
          last_timer_save: null,
          break_lock: {
            target: "locked",
            last_attempt: { action: "enter", status: "succeeded" },
          },
        });
        useTimerObservationStore.setState({
          status: "working",
        });

        useTimerObservationStore.getState().observeTimerTick({
          status: "idle",
          remaining_seconds: 0,
          session_duration_secs: 1500,
          session_type: "work",
          current_cycle: 1,
          completed_pomodoros: 1,
          break_interaction_count: 0,
          away_during_break: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.status).toBe("idle");
        expect(
          selectBreakLockObservation(useTimerObservationStore.getState()),
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

        await expect(setAwayDuringBreak(true)).resolves.toMatchObject({
          status: "rejected",
          error: { code: "invalid_state_transition" },
        });
        expect(
          useTimerOperationStore.getState().operations.set_away_during_break,
        ).toBeDefined();
      });
    });

    describe("observeSessionCompleted()", () => {
      it("should show continue dialog when break completes", () => {
        const { observeSessionCompleted } = useTimerObservationStore.getState();

        observeSessionCompleted({
          transition_reason: "break_waiting_decision",
          completed_session_type: "short_break",
          next_status: "idle",
          next_session_type: "work",
          requires_manual_continue_decision: true,
        });

        const state = useTimerObservationStore.getState();
        expect(state.requiresManualContinueDecision).toBe(true);
      });

      it("should not show continue dialog when work completes", () => {
        const { observeSessionCompleted } = useTimerObservationStore.getState();

        observeSessionCompleted({
          transition_reason: "work_to_break",
          completed_session_type: "work",
          next_status: "on_break",
          next_session_type: "short_break",
          requires_manual_continue_decision: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when break auto-starts next work", () => {
        const { observeSessionCompleted } = useTimerObservationStore.getState();
        useTimerObservationStore.setState({
          requiresManualContinueDecision: true,
        });

        observeSessionCompleted({
          transition_reason: "break_auto_started",
          completed_session_type: "short_break",
          next_status: "working",
          next_session_type: "work",
          requires_manual_continue_decision: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when scheduled stop executes", () => {
        const { observeSessionCompleted } = useTimerObservationStore.getState();
        useTimerObservationStore.setState({
          requiresManualContinueDecision: true,
        });

        observeSessionCompleted({
          transition_reason: "scheduled_stop_executed",
          completed_session_type: "short_break",
          next_status: "idle",
          next_session_type: "work",
          requires_manual_continue_decision: false,
        });

        const state = useTimerObservationStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });
    });

    describe("observeFullState()", () => {
      it("should update all state from full payload", () => {
        const settings = createSettingsForTest({ workDurationSecs: 50 * 60 });

        useTimerObservationStore.getState().observeFullState(
          createFullStatePayload({
            status: "working",
            remaining_seconds: 1200,
            session_duration_secs: 1500,
            current_cycle: 3,
            completed_pomodoros: 2,
            settings: settingsPayloadForTest(settings),
          }),
        );

        const state = useTimerObservationStore.getState();
        expect(state.status).toBe("working");
        expect(state.remainingSeconds).toBe(1200);
        expect(state.sessionDurationSeconds).toBe(1500);
        expect(state.currentCycle).toBe(3);
        expect(state.completedPomodoros).toBe(2);
        expect(state.settings).toEqual(settings);
        expect(state.settingsFileStatus).toBe("ok");
        expect(useTimerSyncStore.getState().connection.status).toBe("idle");
      });

      it("exposes that the settings file could not be used", () => {
        useTimerObservationStore
          .getState()
          .observeFullState(
            createFullStatePayload({ settings_file_status: "unusable" }),
          );

        expect(useTimerObservationStore.getState().settingsFileStatus).toBe(
          "unusable",
        );
      });

      it("keeps timer phase separate from the lock in a full snapshot", () => {
        useTimerObservationStore.getState().observeFullState(
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
        expect(useTimerObservationStore.getState().status).toBe("idle");
        expect(
          selectBreakLockObservation(useTimerObservationStore.getState()),
        ).toBe("locked");
      });

      it("should keep startup prompt state from backend full payload", () => {
        const { observeFullState } = useTimerObservationStore.getState();

        observeFullState(
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

        const state = useTimerObservationStore.getState();
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

    describe("setConnection()", () => {
      it("should update connection status", () => {
        const { setConnection } = useTimerSyncStore.getState();

        expect(useTimerSyncStore.getState().connection.status).toBe("idle");

        setConnection({ status: "ready" });
        expect(useTimerSyncStore.getState().connection.status).toBe("ready");

        setConnection({ status: "idle" });
        expect(useTimerSyncStore.getState().connection.status).toBe("idle");
      });
    });

    describe("Info Message", () => {
      afterEach(() => {
        vi.useRealTimers();
      });

      it("should show stop-pending info message", () => {
        const { showAutoRunStopPendingInfo } = useTimerNoticeStore.getState();

        showAutoRunStopPendingInfo();

        const state = useTimerNoticeStore.getState();
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
          useTimerNoticeStore.getState();

        showAutoRunStopPendingInfo();
        expect(useTimerNoticeStore.getState().infoMessage?.id).toBe(
          "auto_run_stop_pending",
        );
        clearInfoMessage();

        vi.setSystemTime(5999);
        showAutoRunStopPendingInfo();
        expect(useTimerNoticeStore.getState().infoMessage).toBeNull();

        vi.setSystemTime(6000);
        showAutoRunStopPendingInfo();
        expect(useTimerNoticeStore.getState().infoMessage?.id).toBe(
          "auto_run_stop_pending",
        );
      });

      it("should show auto-run stopped info message", () => {
        const { showAutoRunStoppedInfo } = useTimerNoticeStore.getState();

        showAutoRunStoppedInfo();

        const state = useTimerNoticeStore.getState();
        expect(state.infoMessage).not.toBeNull();
        expect(state.infoMessage?.id).toBe("auto_run_stopped");
      });

      it("should clear info message only when expected id matches", () => {
        const { showAutoRunStoppedInfo, clearInfoMessage } =
          useTimerNoticeStore.getState();

        showAutoRunStoppedInfo();
        clearInfoMessage("auto_run_stop_pending");
        expect(useTimerNoticeStore.getState().infoMessage?.id).toBe(
          "auto_run_stopped",
        );

        clearInfoMessage("auto_run_stopped");
        expect(useTimerNoticeStore.getState().infoMessage).toBeNull();
      });
    });
  });
});
