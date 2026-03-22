import { describe, it, expect, beforeEach, vi } from "vitest";
import { useTimerStore, TimerStatus, SessionType } from "./timerStore";
import { useSettingsStore } from "./settingsStore";
import { resetTimerStoreForTest } from "../test/resetTimerStore";

describe("timerStore", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useSettingsStore.getState().resetToDefaults();
  });

  describe("Initial State", () => {
    it("should have correct initial status", () => {
      const state = useTimerStore.getState();
      expect(state.status).toBe(TimerStatus.IDLE);
    });

    it("should have correct initial remaining seconds (25 minutes)", () => {
      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(25 * 60);
    });

    it("should have correct initial session type", () => {
      const state = useTimerStore.getState();
      expect(state.sessionType).toBe(SessionType.WORK);
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
      expect(TimerStatus.IDLE).toBe("idle");
      expect(TimerStatus.WORKING).toBe("working");
      expect(TimerStatus.ON_BREAK).toBe("on_break");
    });

    it("should export SessionType constants", () => {
      expect(SessionType.WORK).toBe("work");
      expect(SessionType.SHORT_BREAK).toBe("short_break");
      expect(SessionType.LONG_BREAK).toBe("long_break");
    });
  });

  describe("Event Handlers", () => {
    describe("handleTimerTick()", () => {
      it("should update state from tick payload", () => {
        const { handleTimerTick } = useTimerStore.getState();

        handleTimerTick({
          status: TimerStatus.WORKING,
          remaining_seconds: 1234,
          session_type: SessionType.WORK,
          current_cycle: 2,
          completed_pomodoros: 1,
          break_interaction_count: 0,
          is_fullscreen_break: false,
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.WORKING);
        expect(state.remainingSeconds).toBe(1234);
        expect(state.sessionType).toBe(SessionType.WORK);
        expect(state.currentCycle).toBe(2);
        expect(state.completedPomodoros).toBe(1);
      });

      it("should update break state", () => {
        const { handleTimerTick } = useTimerStore.getState();

        handleTimerTick({
          status: TimerStatus.ON_BREAK,
          remaining_seconds: 300,
          session_type: SessionType.SHORT_BREAK,
          current_cycle: 1,
          completed_pomodoros: 1,
          break_interaction_count: 3,
          is_fullscreen_break: true,
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.ON_BREAK);
        expect(state.sessionType).toBe(SessionType.SHORT_BREAK);
        expect(state.breakInteractionCount).toBe(3);
        expect(state.isFullscreenBreak).toBe(true);
      });
    });

    describe("handleSessionCompleted()", () => {
      it("should show continue dialog when break completes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();

        handleSessionCompleted({
          transition_reason: "break_waiting_decision",
          completed_session_type: SessionType.SHORT_BREAK,
          next_status: TimerStatus.IDLE,
          next_session_type: SessionType.WORK,
          requires_manual_continue_decision: true,
          post_break_decision_pending: true,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(true);
      });

      it("should not show continue dialog when work completes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();

        handleSessionCompleted({
          transition_reason: "work_to_break",
          completed_session_type: SessionType.WORK,
          next_status: TimerStatus.ON_BREAK,
          next_session_type: SessionType.SHORT_BREAK,
          requires_manual_continue_decision: false,
          post_break_decision_pending: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when break auto-starts next work", () => {
        const { handleSessionCompleted } = useTimerStore.getState();
        useTimerStore.setState({ requiresManualContinueDecision: true });

        handleSessionCompleted({
          transition_reason: "break_auto_started",
          completed_session_type: SessionType.SHORT_BREAK,
          next_status: TimerStatus.WORKING,
          next_session_type: SessionType.WORK,
          requires_manual_continue_decision: false,
          post_break_decision_pending: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });

      it("should keep continue dialog hidden when scheduled stop executes", () => {
        const { handleSessionCompleted } = useTimerStore.getState();
        useTimerStore.setState({ requiresManualContinueDecision: true });

        handleSessionCompleted({
          transition_reason: "scheduled_stop_executed",
          completed_session_type: SessionType.SHORT_BREAK,
          next_status: TimerStatus.IDLE,
          next_session_type: SessionType.WORK,
          requires_manual_continue_decision: false,
          post_break_decision_pending: false,
        });

        const state = useTimerStore.getState();
        expect(state.requiresManualContinueDecision).toBe(false);
      });
    });

    describe("handleFullState()", () => {
      it("should update all state from full payload", () => {
        const { handleFullState } = useTimerStore.getState();

        handleFullState({
          status: TimerStatus.WORKING,
          remaining_seconds: 1500,
          session_type: SessionType.WORK,
          current_cycle: 3,
          completed_pomodoros: 2,
          break_interaction_count: 0,
          is_fullscreen_break: false,
          post_break_decision_pending: false,
          should_prompt_start_on_boot: false,
          active_window_summary: null,
          settings: {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
            post_break_transition_policy: "require_manual_continue",
          },
        });

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.WORKING);
        expect(state.remainingSeconds).toBe(1500);
        expect(state.currentCycle).toBe(3);
        expect(state.completedPomodoros).toBe(2);
        expect(state.isConnected).toBe(true);
      });

      it("should keep startup prompt state from backend full payload", () => {
        const { handleFullState } = useTimerStore.getState();

        handleFullState({
          status: TimerStatus.IDLE,
          remaining_seconds: 1500,
          session_type: SessionType.WORK,
          current_cycle: 1,
          completed_pomodoros: 0,
          break_interaction_count: 0,
          is_fullscreen_break: false,
          post_break_decision_pending: false,
          should_prompt_start_on_boot: true,
          active_window_summary: {
            weekday_key: "mon",
            start_time: "09:00",
            end_time: "18:00",
            crosses_midnight: false,
          },
          settings: {
            work_duration_secs: 1500,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
            post_break_transition_policy: "require_manual_continue",
          },
        });

        const state = useTimerStore.getState();
        expect(state.shouldPromptStartOnBoot).toBe(true);
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
        const { showAutoRunStopPendingInfo } = useTimerStore.getState();
        const dateNowSpy = vi.spyOn(Date, "now");

        dateNowSpy.mockReturnValue(1000);
        showAutoRunStopPendingInfo();
        expect(
          useTimerStore.getState().infoMessageShownAtById.auto_run_stop_pending,
        ).toBe(1000);

        dateNowSpy.mockReturnValue(2000);
        showAutoRunStopPendingInfo();
        expect(
          useTimerStore.getState().infoMessageShownAtById.auto_run_stop_pending,
        ).toBe(1000);

        dateNowSpy.mockReturnValue(7001);
        showAutoRunStopPendingInfo();
        expect(
          useTimerStore.getState().infoMessageShownAtById.auto_run_stop_pending,
        ).toBe(7001);

        dateNowSpy.mockRestore();
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
