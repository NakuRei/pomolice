/**
 * Unit tests for skipFlowStore
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect, beforeEach } from "vitest";
import { useSkipFlowStore, SKIP_FLOW_STEPS, TOTAL_STEPS } from "../skipFlowStore";

describe("skipFlowStore", () => {
  beforeEach(() => {
    // Reset store before each test
    useSkipFlowStore.getState().resetSkipFlow();
  });

  describe("Initial state", () => {
    it("should have correct initial state", () => {
      const state = useSkipFlowStore.getState();

      expect(state.isActive).toBe(false);
      expect(state.currentStep).toBe(1);
      expect(state.stepsCompleted).toEqual(Array(TOTAL_STEPS).fill(false));
      expect(state.stepRetries).toEqual({});
      expect(state.totalTimeSpent).toBe(0);
      expect(state.startedAt).toBeNull();
    });
  });

  describe("startSkipFlow", () => {
    it("should activate skip flow and initialize state", () => {
      const { startSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(true);
      expect(state.currentStep).toBe(1);
      expect(state.startedAt).not.toBeNull();
      expect(state.totalTimeSpent).toBe(0);
    });

    it("should reset state when starting again", () => {
      const store = useSkipFlowStore.getState();

      // Start and progress
      store.startSkipFlow();
      store.nextStep();

      // Start again
      store.startSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.currentStep).toBe(1);
      expect(state.stepsCompleted).toEqual(Array(TOTAL_STEPS).fill(false));
    });
  });

  describe("nextStep", () => {
    it("should advance to next step", () => {
      const { startSkipFlow, nextStep } = useSkipFlowStore.getState();

      startSkipFlow();
      nextStep();

      const state = useSkipFlowStore.getState();
      expect(state.currentStep).toBe(2);
      expect(state.stepsCompleted[0]).toBe(true); // First step completed
    });

    it("should mark steps as completed when advancing", () => {
      const { startSkipFlow, nextStep } = useSkipFlowStore.getState();

      startSkipFlow();

      // Complete first 3 steps
      nextStep(); // Step 1 -> 2
      nextStep(); // Step 2 -> 3
      nextStep(); // Step 3 -> 4

      const state = useSkipFlowStore.getState();
      expect(state.currentStep).toBe(4);
      expect(state.stepsCompleted[0]).toBe(true);
      expect(state.stepsCompleted[1]).toBe(true);
      expect(state.stepsCompleted[2]).toBe(true);
      expect(state.stepsCompleted[3]).toBe(false); // Not completed yet
    });

    it("should trigger completeSkipFlow when reaching final step", () => {
      const { startSkipFlow, nextStep } = useSkipFlowStore.getState();

      startSkipFlow();

      // Advance through all steps
      for (let i = 0; i < TOTAL_STEPS; i++) {
        nextStep();
      }

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(false); // Completed
      expect(state.stepsCompleted[TOTAL_STEPS - 1]).toBe(true);
    });
  });

  describe("retryStep", () => {
    it("should increment retry count for current step", () => {
      const { startSkipFlow, retryStep } = useSkipFlowStore.getState();

      startSkipFlow();
      retryStep();

      const state = useSkipFlowStore.getState();
      expect(state.stepRetries[1]).toBe(1);
    });

    it("should track multiple retries", () => {
      const { startSkipFlow, retryStep } = useSkipFlowStore.getState();

      startSkipFlow();
      retryStep();
      retryStep();
      retryStep();

      const state = useSkipFlowStore.getState();
      expect(state.stepRetries[1]).toBe(3);
    });

    it("should track retries for different steps separately", () => {
      const { startSkipFlow, retryStep, nextStep } = useSkipFlowStore.getState();

      startSkipFlow();

      // Retry step 1 twice
      retryStep();
      retryStep();

      // Move to step 2 and retry once
      nextStep();
      retryStep();

      const state = useSkipFlowStore.getState();
      expect(state.stepRetries[1]).toBe(2);
      expect(state.stepRetries[2]).toBe(1);
    });
  });

  describe("cancelSkipFlow", () => {
    it("should reset to initial state", () => {
      const { startSkipFlow, nextStep, retryStep, cancelSkipFlow } =
        useSkipFlowStore.getState();

      // Make some progress
      startSkipFlow();
      nextStep();
      nextStep();
      retryStep();

      // Cancel
      cancelSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(false);
      expect(state.currentStep).toBe(1);
      expect(state.stepsCompleted).toEqual(Array(TOTAL_STEPS).fill(false));
      expect(state.stepRetries).toEqual({});
      expect(state.startedAt).toBeNull();
    });
  });

  describe("completeSkipFlow", () => {
    it("should mark as inactive and complete final step", () => {
      const { startSkipFlow, completeSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();
      completeSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(false);
      expect(state.stepsCompleted[TOTAL_STEPS - 1]).toBe(true);
    });

    it("should calculate total time spent", () => {
      const { startSkipFlow, completeSkipFlow } = useSkipFlowStore.getState();

      startSkipFlow();

      // Wait a bit (simulate time passing)
      // Note: In real tests, we would mock Date.now()
      const startTime = Date.now();

      // Simulate 1 second delay
      while (Date.now() - startTime < 1000) {
        // Busy wait
      }

      completeSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBeGreaterThan(0);
    });
  });

  describe("updateTimeSpent", () => {
    it("should not update if not active", () => {
      const { updateTimeSpent } = useSkipFlowStore.getState();

      updateTimeSpent();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBe(0);
    });

    it("should update time spent when active", () => {
      const { startSkipFlow, updateTimeSpent } = useSkipFlowStore.getState();

      startSkipFlow();

      // Wait a bit
      const startTime = Date.now();
      while (Date.now() - startTime < 1000) {
        // Busy wait
      }

      updateTimeSpent();

      const state = useSkipFlowStore.getState();
      expect(state.totalTimeSpent).toBeGreaterThan(0);
    });
  });

  describe("resetSkipFlow", () => {
    it("should completely reset store to initial state", () => {
      const { startSkipFlow, nextStep, retryStep, resetSkipFlow } =
        useSkipFlowStore.getState();

      // Make progress
      startSkipFlow();
      nextStep();
      nextStep();
      retryStep();

      // Reset
      resetSkipFlow();

      const state = useSkipFlowStore.getState();
      expect(state).toMatchObject({
        isActive: false,
        currentStep: 1,
        stepsCompleted: Array(TOTAL_STEPS).fill(false),
        stepRetries: {},
        totalTimeSpent: 0,
        startedAt: null,
      });
    });
  });

  describe("SKIP_FLOW_STEPS constants", () => {
    it("should have correct step constants", () => {
      expect(SKIP_FLOW_STEPS.CONFIRMATION_1).toBe(1);
      expect(SKIP_FLOW_STEPS.CONFIRMATION_2).toBe(2);
      expect(SKIP_FLOW_STEPS.CONFIRMATION_3).toBe(3);
      expect(SKIP_FLOW_STEPS.CONFIRMATION_4).toBe(4);
      expect(SKIP_FLOW_STEPS.TYPING_CHALLENGE_1).toBe(5);
      expect(SKIP_FLOW_STEPS.MATH_CHALLENGE_1).toBe(6);
      expect(SKIP_FLOW_STEPS.TYPING_CHALLENGE_2).toBe(7);
      expect(SKIP_FLOW_STEPS.MATH_CHALLENGE_2).toBe(8);
      expect(SKIP_FLOW_STEPS.LONG_PRESS).toBe(9);
      expect(SKIP_FLOW_STEPS.FINAL_WARNING).toBe(10);
    });

    it("should have correct total steps", () => {
      expect(TOTAL_STEPS).toBe(10);
    });
  });
});
