import { type TimerEffectsState, commands } from "@/ipc/generated";
import { create } from "zustand";
import { classifyBackendError } from "@/errors/classifyBackendError";
import type { BreakLockObservation } from "@/types/timerEffects";

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
    if (
      current === null ||
      BigInt(effects.revision) > BigInt(current.revision)
    ) {
      set({ effects, retryError: null });
    }
  },
  retry: async (): Promise<void> => {
    if (get().retrying) return;
    set({ retrying: true, retryError: null });
    try {
      const outcome = await commands.retryFailedTimerEffects();
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
