import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

/**
 * Timer status constants
 * Using const object for type-safe, reusable constants with zero runtime overhead
 */
export const TimerStatus = {
  IDLE: "idle",
  RUNNING: "running",
  PAUSED: "paused",
} as const;

export type TimerStatusType = typeof TimerStatus[keyof typeof TimerStatus];

/**
 * Session type constants
 */
export const SessionType = {
  WORK: "work",
  BREAK: "break",
} as const;

export type SessionTypeType = typeof SessionType[keyof typeof SessionType];

/**
 * Timer state interface
 * Phase 0: Basic structure for timer state management
 */
interface TimerState {
  // Timer status: idle (not started), running (counting down), paused (not used in Phase 0)
  status: TimerStatusType;

  // Remaining time in seconds
  remainingSeconds: number;

  // Session type: work or break
  sessionType: SessionTypeType;
}

/**
 * Timer actions interface
 * Phase 0: Basic actions for timer control
 */
interface TimerActions {
  // Start the timer
  start: () => void;

  // Stop the timer
  stop: () => void;

  // Reset the timer to initial state
  reset: () => void;

  // Tick function to decrease time by 1 second
  tick: () => void;
}

/**
 * Combined timer store type
 */
type TimerStore = TimerState & TimerActions;

/**
 * Initial state for the timer
 */
const initialState: TimerState = {
  status: TimerStatus.IDLE,
  remainingSeconds: 25 * 60, // 25 minutes in seconds
  sessionType: SessionType.WORK,
};

/**
 * Zustand store for timer state management
 * Uses immer middleware for immutable state updates
 * Uses devtools middleware for Redux DevTools integration
 */
export const useTimerStore = create<TimerStore>()(
  devtools(
    immer((set) => ({
      ...initialState,

      start: () =>
        set((state) => {
          state.status = TimerStatus.RUNNING;
        }),

      stop: () =>
        set((state) => {
          state.status = TimerStatus.IDLE;
        }),

      reset: () => set(initialState),

      tick: () =>
        set((state) => {
          if (state.status === TimerStatus.RUNNING && state.remainingSeconds > 0) {
            state.remainingSeconds -= 1;
          }
        }),
    })),
    { name: "TimerStore" }
  )
);
