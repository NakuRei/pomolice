import { useTimerStore } from "@/store/timerStore";
import { useTimerEffectsStore } from "@/store/timerEffectsStore";

export function resetTimerStoreForTest(): void {
  useTimerStore.setState(useTimerStore.getInitialState(), true);
  useTimerEffectsStore.setState(useTimerEffectsStore.getInitialState(), true);
}
