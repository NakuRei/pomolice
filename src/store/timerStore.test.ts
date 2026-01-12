import { describe, it, expect, beforeEach } from "vitest";
import { useTimerStore, TimerStatus, SessionType } from "./timerStore";

describe("timerStore", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
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
      expect(SessionType.BREAK).toBe("break");
    });
  });
});
