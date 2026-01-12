import { describe, it, expect, beforeEach } from "vitest";
import { useTimerStore, TimerStatus, SessionType } from "../timerStore";
import { useSettingsStore } from "../settingsStore";

describe("timerStore", () => {
  beforeEach(() => {
    // Reset stores to initial state before each test
    useTimerStore.getState().reset();
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
      expect(state.showContinueDialog).toBe(false);
    });
  });

  describe("Actions", () => {
    describe("start()", () => {
      it("should change status to RUNNING", () => {
        const { start } = useTimerStore.getState();
        start();
        expect(useTimerStore.getState().status).toBe(TimerStatus.RUNNING);
      });

      it("should not change remaining seconds", () => {
        const initialSeconds = useTimerStore.getState().remainingSeconds;
        const { start } = useTimerStore.getState();
        start();
        expect(useTimerStore.getState().remainingSeconds).toBe(initialSeconds);
      });
    });

    describe("stop()", () => {
      it("should change status to IDLE", () => {
        const { start, stop } = useTimerStore.getState();
        start();
        stop();
        expect(useTimerStore.getState().status).toBe(TimerStatus.IDLE);
      });
    });

    describe("reset()", () => {
      it("should reset to initial state", () => {
        const { start, tick, reset } = useTimerStore.getState();
        start();
        tick();
        tick();
        reset();

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.IDLE);
        expect(state.remainingSeconds).toBe(25 * 60);
        expect(state.sessionType).toBe(SessionType.WORK);
      });
    });

    describe("tick()", () => {
      it("should decrease remaining seconds by 1 when status is RUNNING", () => {
        const { start, tick } = useTimerStore.getState();
        start();
        const initialSeconds = useTimerStore.getState().remainingSeconds;
        tick();
        expect(useTimerStore.getState().remainingSeconds).toBe(
          initialSeconds - 1
        );
      });

      it("should not decrease remaining seconds when status is IDLE", () => {
        const { tick } = useTimerStore.getState();
        const initialSeconds = useTimerStore.getState().remainingSeconds;
        tick();
        expect(useTimerStore.getState().remainingSeconds).toBe(initialSeconds);
      });

      it("should not decrease remaining seconds below 0", () => {
        const { start, reset } = useTimerStore.getState();
        // Set remaining seconds to 0
        reset();
        useTimerStore.setState({ remainingSeconds: 0 });
        start();

        const { tick } = useTimerStore.getState();
        tick();
        expect(useTimerStore.getState().remainingSeconds).toBe(0);
      });

      it("should decrease multiple times correctly", () => {
        const { start, tick } = useTimerStore.getState();
        start();
        const initialSeconds = useTimerStore.getState().remainingSeconds;

        tick();
        tick();
        tick();

        expect(useTimerStore.getState().remainingSeconds).toBe(
          initialSeconds - 3
        );
      });
    });
  });

  describe("Constants", () => {
    it("should export TimerStatus constants", () => {
      expect(TimerStatus.IDLE).toBe("idle");
      expect(TimerStatus.RUNNING).toBe("running");
      expect(TimerStatus.PAUSED).toBe("paused");
    });

    it("should export SessionType constants", () => {
      expect(SessionType.WORK).toBe("work");
      expect(SessionType.SHORT_BREAK).toBe("short_break");
      expect(SessionType.LONG_BREAK).toBe("long_break");
    });
  });

  describe("Phase 2: Cycle Management", () => {
    describe("start() - cycle initialization", () => {
      it("should initialize cycle to 1 on first start", () => {
        const { start } = useTimerStore.getState();
        expect(useTimerStore.getState().currentCycle).toBe(0);
        start();
        expect(useTimerStore.getState().currentCycle).toBe(1);
      });
    });

    describe("completeSession() - work to break transition", () => {
      it("should transition from work to short break after work completion", () => {
        const state = useTimerStore.getState();
        state.start();

        // Simulate work session completion
        useTimerStore.setState({
          remainingSeconds: 0,
          currentCycle: 1,
          sessionType: SessionType.WORK
        });

        state.completeSession();

        const newState = useTimerStore.getState();
        expect(newState.sessionType).toBe(SessionType.SHORT_BREAK);
        expect(newState.remainingSeconds).toBe(5 * 60); // 5 minutes default
        expect(newState.status).toBe(TimerStatus.RUNNING);
        expect(newState.completedPomodoros).toBe(1);
      });

      it("should transition to long break after 4th work session", () => {
        const state = useTimerStore.getState();

        // Set up state for 4th work session
        useTimerStore.setState({
          currentCycle: 4,
          sessionType: SessionType.WORK,
          status: TimerStatus.RUNNING,
          remainingSeconds: 0,
        });

        state.completeSession();

        const newState = useTimerStore.getState();
        expect(newState.sessionType).toBe(SessionType.LONG_BREAK);
        expect(newState.remainingSeconds).toBe(15 * 60); // 15 minutes default
      });

      it("should transition to short break for cycles 1, 2, 3", () => {
        const cycles = [1, 2, 3];

        cycles.forEach((cycle) => {
          useTimerStore.getState().reset();
          useTimerStore.setState({
            currentCycle: cycle,
            sessionType: SessionType.WORK,
            remainingSeconds: 0,
          });

          useTimerStore.getState().completeSession();

          expect(useTimerStore.getState().sessionType).toBe(SessionType.SHORT_BREAK);
        });
      });
    });

    describe("completeSession() - break completion", () => {
      it("should stop timer and show continue dialog after short break", () => {
        useTimerStore.setState({
          sessionType: SessionType.SHORT_BREAK,
          remainingSeconds: 0,
          status: TimerStatus.RUNNING,
          currentCycle: 1,
        });

        useTimerStore.getState().completeSession();

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.IDLE);
        expect(state.showContinueDialog).toBe(true);
        expect(state.currentCycle).toBe(1); // Should not change
      });

      it("should reset cycle after long break completion", () => {
        useTimerStore.setState({
          sessionType: SessionType.LONG_BREAK,
          remainingSeconds: 0,
          status: TimerStatus.RUNNING,
          currentCycle: 4,
        });

        useTimerStore.getState().completeSession();

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.IDLE);
        expect(state.showContinueDialog).toBe(true);
        expect(state.currentCycle).toBe(0); // Reset after long break
      });
    });

    describe("startNextSession()", () => {
      it("should start next work session and increment cycle", () => {
        useTimerStore.setState({
          status: TimerStatus.IDLE,
          showContinueDialog: true,
          currentCycle: 1,
          sessionType: SessionType.SHORT_BREAK,
        });

        useTimerStore.getState().startNextSession();

        const state = useTimerStore.getState();
        expect(state.sessionType).toBe(SessionType.WORK);
        expect(state.currentCycle).toBe(2);
        expect(state.remainingSeconds).toBe(25 * 60);
        expect(state.status).toBe(TimerStatus.RUNNING);
        expect(state.showContinueDialog).toBe(false);
      });
    });

    describe("endCycle()", () => {
      it("should end cycle and return to idle state", () => {
        useTimerStore.setState({
          status: TimerStatus.IDLE,
          showContinueDialog: true,
          currentCycle: 2,
          completedPomodoros: 2,
          sessionType: SessionType.SHORT_BREAK,
        });

        useTimerStore.getState().endCycle();

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.IDLE);
        expect(state.sessionType).toBe(SessionType.WORK);
        expect(state.remainingSeconds).toBe(25 * 60);
        expect(state.showContinueDialog).toBe(false);
        expect(state.completedPomodoros).toBe(2); // Should keep count
        expect(state.currentCycle).toBe(2); // Should keep count
      });
    });

    describe("emergencyStop()", () => {
      it("should reset to idle state", () => {
        useTimerStore.setState({
          status: TimerStatus.RUNNING,
          sessionType: SessionType.WORK,
          remainingSeconds: 100,
          currentCycle: 2,
          completedPomodoros: 1,
        });

        useTimerStore.getState().emergencyStop();

        const state = useTimerStore.getState();
        expect(state.status).toBe(TimerStatus.IDLE);
        expect(state.sessionType).toBe(SessionType.WORK);
        expect(state.remainingSeconds).toBe(25 * 60); // Reset to default
        expect(state.showContinueDialog).toBe(false);
        // Counts are preserved for statistics
        expect(state.completedPomodoros).toBe(1);
        expect(state.currentCycle).toBe(2);
      });
    });
  });
});
