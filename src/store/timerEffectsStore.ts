import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { classifyBackendError } from "@/errors/classifyBackendError";
import type {
  BreakLockObservation,
  TimerEffectsState,
  TimerSnapshot,
} from "@/types/timerEffects";

interface TimerEffectsStore {
  effects: TimerEffectsState | null;
  retrying: boolean;
  retryError: string | null;
  observe: (effects: TimerEffectsState) => void;
  retry: () => Promise<void>;
}

export const useTimerEffectsStore = create<TimerEffectsStore>((set, get) => ({
  effects: null,
  retrying: false,
  retryError: null,
  observe: (effects): void => {
    const current = get().effects;
    if (current === null || effects.revision > current.revision) {
      set({ effects, retryError: null });
    }
  },
  retry: async (): Promise<void> => {
    if (get().retrying) return;
    set({ retrying: true, retryError: null });
    try {
      const outcome = await invoke<TimerSnapshot>("retry_failed_timer_effects");
      get().observe(outcome.effects);
    } catch (error) {
      set({ retryError: classifyBackendError(error).userMessage });
    } finally {
      set({ retrying: false });
    }
  },
}));

export function selectBreakLockObservation(
  state: Pick<TimerEffectsStore, "effects">,
): BreakLockObservation {
  const attempt = state.effects?.break_lock.last_attempt;
  if (!attempt || attempt.status === "failed") return "unknown";
  return attempt.action === "enter" ? "locked" : "unlocked";
}
