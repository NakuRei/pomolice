import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

/**
 * Skip flow step types (1-10)
 * Phase 5: Multi-step skip confirmation flow
 *
 * Steps:
 * 1-4: Confirmation dialogs (Yes/No)
 * 5: Typing challenge 1 (short phrase)
 * 6: Math challenge 1 (easy)
 * 7: Typing challenge 2 (long phrase)
 * 8: Math challenge 2 (hard)
 * 9: Long press button (15 seconds)
 * 10: Final warning with SNS notification
 */
export const SKIP_FLOW_STEPS = {
  CONFIRMATION_1: 1,
  CONFIRMATION_2: 2,
  CONFIRMATION_3: 3,
  CONFIRMATION_4: 4,
  TYPING_CHALLENGE_1: 5,
  MATH_CHALLENGE_1: 6,
  TYPING_CHALLENGE_2: 7,
  MATH_CHALLENGE_2: 8,
  LONG_PRESS: 9,
  FINAL_WARNING: 10,
} as const;

export const TOTAL_STEPS = 10;

/**
 * Skip flow state interface
 */
interface SkipFlowState {
  // Whether skip flow is currently active
  isActive: boolean;

  // Current step (1-10)
  currentStep: number;

  // Completion status for each step
  stepsCompleted: boolean[];

  // Number of retries for each step
  stepRetries: Record<number, number>;

  // Total time spent in skip flow (seconds)
  totalTimeSpent: number;

  // Timestamp when skip flow started
  startedAt: number | null;
}

/**
 * Skip flow actions interface
 */
interface SkipFlowActions {
  // Start the skip flow
  startSkipFlow: () => void;

  // Advance to next step
  nextStep: () => void;

  // Retry current step (failed validation)
  retryStep: () => void;

  // Cancel skip flow and return to break
  cancelSkipFlow: () => void;

  // Complete the skip flow (all 10 steps done)
  completeSkipFlow: () => void;

  // Update total time spent (called on tick)
  updateTimeSpent: () => void;

  // Reset skip flow to initial state
  resetSkipFlow: () => void;
}

/**
 * Combined skip flow store type
 */
type SkipFlowStore = SkipFlowState & SkipFlowActions;

/**
 * Initial state for skip flow
 */
const initialState: SkipFlowState = {
  isActive: false,
  currentStep: 1,
  stepsCompleted: Array(TOTAL_STEPS).fill(false),
  stepRetries: {},
  totalTimeSpent: 0,
  startedAt: null,
};

/**
 * Helper to conditionally apply devtools middleware only in development
 */
const noopDevtools = (<T,>(config: T): T => config) as typeof devtools;
const conditionalDevtools = import.meta.env.DEV ? devtools : noopDevtools;

/**
 * Zustand store for skip flow state management
 * Uses immer middleware for immutable state updates
 * Uses devtools middleware for Redux DevTools integration (development only)
 */
export const useSkipFlowStore = create<SkipFlowStore>()(
  conditionalDevtools(
    immer((set, get) => ({
      ...initialState,

      startSkipFlow: () =>
        set((state) => {
          state.isActive = true;
          state.currentStep = 1;
          state.stepsCompleted = Array(TOTAL_STEPS).fill(false);
          state.stepRetries = {};
          state.totalTimeSpent = 0;
          state.startedAt = Date.now();
        }),

      nextStep: () => {
        set((draft) => {
          // Mark current step as completed
          draft.stepsCompleted[draft.currentStep - 1] = true;

          // Move to next step
          draft.currentStep += 1;
        });

        // Check if we've moved past the last step
        const newState = get();
        if (newState.currentStep > TOTAL_STEPS) {
          // All steps completed, trigger complete
          get().completeSkipFlow();
        }
      },

      retryStep: () => {
        set((draft) => {
          // Increment retry count for current step
          const currentRetries = draft.stepRetries[draft.currentStep] || 0;
          draft.stepRetries[draft.currentStep] = currentRetries + 1;
        });
      },

      cancelSkipFlow: () =>
        set((state) => {
          // Reset to initial state
          state.isActive = false;
          state.currentStep = 1;
          state.stepsCompleted = Array(TOTAL_STEPS).fill(false);
          state.stepRetries = {};
          state.totalTimeSpent = 0;
          state.startedAt = null;
        }),

      completeSkipFlow: () => {
        set((draft) => {
          // Mark final step as completed
          draft.stepsCompleted[TOTAL_STEPS - 1] = true;

          // Calculate final time spent
          if (draft.startedAt !== null) {
            draft.totalTimeSpent = Math.floor((Date.now() - draft.startedAt) / 1000);
          }

          // Mark as inactive (will be reset externally)
          draft.isActive = false;
        });

        // Note: Actual skip logic (exiting break, recording statistics)
        // will be handled by the component that calls this action
      },

      updateTimeSpent: () => {
        const state = get();

        if (!state.isActive || state.startedAt === null) {
          return;
        }

        set((draft) => {
          draft.totalTimeSpent = Math.floor((Date.now() - draft.startedAt!) / 1000);
        });
      },

      resetSkipFlow: () => set(initialState),
    })),
    { name: "SkipFlowStore" }
  )
);
