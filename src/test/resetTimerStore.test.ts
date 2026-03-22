import { beforeEach, describe, expect, it } from "vitest";
import { TimerStatus, SessionType, useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "./resetTimerStore";

describe("resetTimerStoreForTest", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
  });

  it("should restore timer state to initial values", () => {
    useTimerStore.setState({
      status: TimerStatus.ON_BREAK,
      remainingSeconds: 42,
      sessionType: SessionType.LONG_BREAK,
      currentCycle: 3,
      completedPomodoros: 2,
      breakInteractionCount: 4,
      isFullscreenBreak: true,
      requiresManualContinueDecision: true,
      isConnected: true,
      shouldPromptStartOnBoot: true,
      activeWindowSummary: {
        weekday_key: "mon",
        start_time: "09:00",
        end_time: "18:00",
        crosses_midnight: false,
      },
      infoMessage: {
        id: "auto_run_stopped",
        message: "custom",
        ttlMs: 1,
      },
      infoMessageShownAtById: {
        auto_run_stopped: 1234,
      },
    });

    resetTimerStoreForTest();

    const state = useTimerStore.getState();
    expect(state.status).toBe(TimerStatus.IDLE);
    expect(state.remainingSeconds).toBe(25 * 60);
    expect(state.sessionType).toBe(SessionType.WORK);
    expect(state.currentCycle).toBe(0);
    expect(state.completedPomodoros).toBe(0);
    expect(state.breakInteractionCount).toBe(0);
    expect(state.isFullscreenBreak).toBe(false);
    expect(state.requiresManualContinueDecision).toBe(false);
    expect(state.isConnected).toBe(false);
    expect(state.shouldPromptStartOnBoot).toBe(false);
    expect(state.activeWindowSummary).toBeNull();
    expect(state.infoMessage).toBeNull();
    expect(state.infoMessageShownAtById).toEqual({});
  });

  it("should keep timer actions callable after reset", () => {
    useTimerStore.setState({
      status: TimerStatus.WORKING,
      currentCycle: 2,
    });

    resetTimerStoreForTest();

    const state = useTimerStore.getState();
    expect(state.start).toBeTypeOf("function");
    expect(state.resolvePostBreakDecision).toBeTypeOf("function");
    expect(state.emergencyStop).toBeTypeOf("function");
    expect(state.syncSettings).toBeTypeOf("function");
    expect(state.handleTimerTick).toBeTypeOf("function");
    expect(state.handleSessionCompleted).toBeTypeOf("function");
    expect(state.handleFullState).toBeTypeOf("function");
    expect(state.setConnected).toBeTypeOf("function");
  });
});
