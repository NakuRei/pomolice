import { useTimerObservationStore } from "@/store/timerObservationStore";
import { useTimerOperationStore } from "@/operations/operationStore";
import { useTimerSyncStore } from "@/store/timerSyncStore";
import { useTimerNoticeStore } from "@/store/timerNoticeStore";
export function resetTimerStoreForTest(): void {
  useTimerObservationStore.setState(
    useTimerObservationStore.getInitialState(),
    true,
  );
  useTimerOperationStore.setState(
    useTimerOperationStore.getInitialState(),
    true,
  );
  useTimerSyncStore.setState(useTimerSyncStore.getInitialState(), true);
  useTimerNoticeStore.setState(useTimerNoticeStore.getInitialState(), true);
}
