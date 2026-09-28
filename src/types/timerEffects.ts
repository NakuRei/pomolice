import type { FullStatePayload } from "./timer";

export type BreakLockObservation = "locked" | "unlocked" | "unknown";

export type EffectStatus = "succeeded" | "failed";

export interface TimerEffectsState {
  revision: number;
  break_lock: {
    target: "locked" | "unlocked";
    last_attempt: {
      action: "enter" | "release";
      status: EffectStatus;
    } | null;
  };
  last_timer_save: EffectStatus | null;
}

export interface TimerSnapshot {
  timer: FullStatePayload;
  effects: TimerEffectsState;
}
