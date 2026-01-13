import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import { useSettingsStore } from "./settingsStore";
import { notifyWorkComplete, notifyBreakComplete, notifyLongBreakComplete } from "../services/notifications";

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
  SHORT_BREAK: "short_break",
  LONG_BREAK: "long_break",
} as const;

export type SessionTypeType = typeof SessionType[keyof typeof SessionType];

/**
 * Timer state interface
 * Phase 2: Extended with cycle tracking and session management
 */
interface TimerState {
  // Timer status: idle (not started), running (counting down), paused (not used in Phase 1-2)
  status: TimerStatusType;

  // Remaining time in seconds
  remainingSeconds: number;

  // Session type: work, short_break, or long_break
  sessionType: SessionTypeType;

  // Number of completed pomodoros (work sessions) in current cycle
  completedPomodoros: number;

  // Current cycle position (1-based, resets after long break)
  currentCycle: number;

  // Flag to show "Continue?" dialog after break completion
  showContinueDialog: boolean;
}

/**
 * Timer actions interface
 * Phase 2: Extended with session completion and cycle management
 */
interface TimerActions {
  // Start the timer
  start: () => void;

  // Stop the timer (Phase 1 only - will be removed in Phase 2)
  stop: () => void;

  // Emergency stop (Phase 2+)
  emergencyStop: () => void;

  // Reset the timer to initial state
  reset: () => void;

  // Tick function to decrease time by 1 second
  tick: () => void;

  // Complete current session and transition to next
  completeSession: () => void;

  // Start next work session (called from continue dialog)
  startNextSession: () => void;

  // End pomodoro cycle (called from continue dialog)
  endCycle: () => void;
}

/**
 * Combined timer store type
 */
type TimerStore = TimerState & TimerActions;

/**
 * Get initial remaining seconds from settings
 */
function getInitialRemainingSeconds(): number {
  return useSettingsStore.getState().workDuration * 60;
}

/**
 * Initial state for the timer
 * Phase 2: Extended with cycle tracking
 */
const initialState: TimerState = {
  status: TimerStatus.IDLE,
  remainingSeconds: getInitialRemainingSeconds(),
  sessionType: SessionType.WORK,
  completedPomodoros: 0,
  currentCycle: 0,
  showContinueDialog: false,
};

/**
 * Determine break type based on current cycle
 * @param cycle - Current cycle number (1-based)
 * @param interval - Long break interval from settings
 * @returns "short_break" or "long_break"
 */
function getBreakType(cycle: number, interval: number): SessionTypeType {
  return cycle % interval === 0 ? SessionType.LONG_BREAK : SessionType.SHORT_BREAK;
}

/**
 * Zustand store for timer state management
 * Uses immer middleware for immutable state updates
 * Uses devtools middleware for Redux DevTools integration
 */
export const useTimerStore = create<TimerStore>()(
  devtools(
    immer((set, get) => ({
      ...initialState,

      start: () =>
        set((state) => {
          state.status = TimerStatus.RUNNING;
          // Initialize cycle on first start if needed
          if (state.currentCycle === 0) {
            state.currentCycle = 1;
          }
        }),

      stop: () =>
        set((state) => {
          state.status = TimerStatus.IDLE;
        }),

      emergencyStop: () =>
        set((state) => {
          // Emergency stop: invalidate session, reset to idle
          state.status = TimerStatus.IDLE;
          state.sessionType = SessionType.WORK;
          state.remainingSeconds = useSettingsStore.getState().workDuration * 60;
          state.showContinueDialog = false;
          // Note: completedPomodoros and currentCycle are NOT reset
          // to maintain the count, but current session is invalidated
        }),

      reset: () => set(initialState),

      tick: () => {
        const state = get();
        if (state.status === TimerStatus.RUNNING && state.remainingSeconds > 0) {
          set((draft) => {
            draft.remainingSeconds -= 1;
          });

          // Check if session is complete after decrement
          if (state.remainingSeconds === 1) {
            // Will become 0 after this tick, so complete the session
            get().completeSession();
          }
        }
      },

      completeSession: () =>
        set((state) => {
          const settings = useSettingsStore.getState();

          if (state.sessionType === SessionType.WORK) {
            // Work session completed
            state.completedPomodoros += 1;

            // Determine break type
            const breakType = getBreakType(state.currentCycle, settings.longBreakInterval);
            state.sessionType = breakType;

            // Set break duration
            state.remainingSeconds =
              breakType === SessionType.LONG_BREAK
                ? settings.longBreakDuration * 60
                : settings.shortBreakDuration * 60;

            // Auto-start break (no user interaction needed)
            state.status = TimerStatus.RUNNING;

            // Show notification
            notifyWorkComplete();
          } else {
            // Break session completed (short or long)
            const isLongBreak = state.sessionType === SessionType.LONG_BREAK;

            state.status = TimerStatus.IDLE;
            state.showContinueDialog = true;

            // If long break completed, reset cycle
            if (isLongBreak) {
              state.currentCycle = 0;
            }

            // Show notification
            if (isLongBreak) {
              notifyLongBreakComplete();
            } else {
              notifyBreakComplete();
            }
          }
        }),

      startNextSession: () =>
        set((state) => {
          const settings = useSettingsStore.getState();

          // Increment cycle for next work session
          state.currentCycle += 1;
          state.sessionType = SessionType.WORK;
          state.remainingSeconds = settings.workDuration * 60;
          state.showContinueDialog = false;
          state.status = TimerStatus.RUNNING;
        }),

      endCycle: () =>
        set((state) => {
          const settings = useSettingsStore.getState();

          // End cycle, return to idle
          state.status = TimerStatus.IDLE;
          state.sessionType = SessionType.WORK;
          state.remainingSeconds = settings.workDuration * 60;
          state.showContinueDialog = false;
          // Keep completedPomodoros and currentCycle for statistics
        }),
    })),
    { name: "TimerStore" }
  )
);

// Subscribe to settings changes and update timer when idle
useSettingsStore.subscribe((settings, prevSettings) => {
  const timerState = useTimerStore.getState();

  // Only update if timer is idle
  if (timerState.status !== TimerStatus.IDLE) {
    return;
  }

  // Update remaining seconds based on current session type
  let newRemainingSeconds = timerState.remainingSeconds;

  if (timerState.sessionType === SessionType.WORK && settings.workDuration !== prevSettings.workDuration) {
    newRemainingSeconds = settings.workDuration * 60;
  } else if (timerState.sessionType === SessionType.SHORT_BREAK && settings.shortBreakDuration !== prevSettings.shortBreakDuration) {
    newRemainingSeconds = settings.shortBreakDuration * 60;
  } else if (timerState.sessionType === SessionType.LONG_BREAK && settings.longBreakDuration !== prevSettings.longBreakDuration) {
    newRemainingSeconds = settings.longBreakDuration * 60;
  }

  if (newRemainingSeconds !== timerState.remainingSeconds) {
    useTimerStore.setState({ remainingSeconds: newRemainingSeconds });
  }
});
