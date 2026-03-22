import { useTimerStore } from "@/store/timerStore";

export function resetTimerStoreForTest(): void {
  useTimerStore.setState(useTimerStore.getInitialState(), true);
}
