/**
 * Unit tests for skipFlowStore
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { useSkipFlowStore } from "./skipFlowStore";

describe("skipFlowStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));

    // Reset store before each test
    useSkipFlowStore.getState().resetSkipFlow();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("Initial state", () => {
    it("starts inactive with no elapsed time", () => {
      const state = useSkipFlowStore.getState();

      expect(state.isActive).toBe(false);
      expect(state.totalTimeSpent).toBe(0);
    });
  });

  describe("startSkipFlow", () => {
    it("activates the skip flow with no elapsed time", () => {
      const { startSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(true);
      expect(state.totalTimeSpent).toBe(0);
    });

    it("resets elapsed time when starting again after earlier progress", () => {
      const store = useSkipFlowStore.getState();

      store.startSkipFlow();
      vi.setSystemTime(new Date("2026-01-01T00:00:01Z"));
      store.updateTimeSpent();
      store.nextStep();
      store.startSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(true);
      expect(state.totalTimeSpent).toBe(0);
    });
  });

  describe("nextStep", () => {
    it("does not complete the skip flow immediately", () => {
      const { startSkipFlow, nextStep } = useSkipFlowStore.getState();

      startSkipFlow();
      nextStep();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(true);
    });
  });

  describe("cancelSkipFlow", () => {
    it("cancels the active skip flow and lets the next attempt start over", () => {
      const { startSkipFlow, nextStep, cancelSkipFlow } =
        useSkipFlowStore.getState();

      startSkipFlow();
      nextStep();
      nextStep();
      cancelSkipFlow();

      expect(useSkipFlowStore.getState().isActive).toBe(false);

      startSkipFlow();

      const restartedState = useSkipFlowStore.getState();
      expect(restartedState.isActive).toBe(true);
      expect(restartedState.totalTimeSpent).toBe(0);
    });
  });

  describe("completeSkipFlow", () => {
    it("marks the skip flow as inactive", () => {
      const { startSkipFlow, completeSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();
      completeSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(false);
    });

    it("calculates total time spent", () => {
      const { startSkipFlow, completeSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();
      vi.setSystemTime(new Date("2026-01-01T00:00:01Z"));

      completeSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBe(1);
    });
  });

  describe("updateTimeSpent", () => {
    it("does not update elapsed time while inactive", () => {
      const { updateTimeSpent } = useSkipFlowStore.getState();

      updateTimeSpent();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBe(0);
    });

    it("updates elapsed time while active", () => {
      const { startSkipFlow, updateTimeSpent } = useSkipFlowStore.getState();

      startSkipFlow();
      vi.setSystemTime(new Date("2026-01-01T00:00:01Z"));

      updateTimeSpent();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBe(1);
    });
  });

  describe("resetSkipFlow", () => {
    it("clears the active flow and lets the next attempt start over", () => {
      const { startSkipFlow, nextStep, resetSkipFlow } =
        useSkipFlowStore.getState();

      startSkipFlow();
      nextStep();
      nextStep();
      resetSkipFlow();

      expect(useSkipFlowStore.getState().isActive).toBe(false);

      startSkipFlow();

      const restartedState = useSkipFlowStore.getState();
      expect(restartedState.isActive).toBe(true);
      expect(restartedState.totalTimeSpent).toBe(0);
    });
  });
});
