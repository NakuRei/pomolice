import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "@/backend/operationStore";
import { useBackendSyncStore } from "@/backend/syncStore";
import { useTimerNoticeStore } from "@/features/timer/noticeStore";
import { useEmergencyShortcutStore } from "@/backend/emergencyShortcutStore";
import { useInputMonitorStore } from "@/backend/inputMonitorStore";
import { useTrayStore } from "@/backend/trayStore";
export function resetAppStateForTest(): void {
  useEmergencyShortcutStore.setState(
    useEmergencyShortcutStore.getInitialState(),
    true,
  );
  useInputMonitorStore.setState(useInputMonitorStore.getInitialState(), true);
  useTrayStore.setState(useTrayStore.getInitialState(), true);
  useBackendObservationStore.setState(
    useBackendObservationStore.getInitialState(),
    true,
  );
  useOperationStore.setState(useOperationStore.getInitialState(), true);
  useBackendSyncStore.setState(useBackendSyncStore.getInitialState(), true);
  useTimerNoticeStore.setState(useTimerNoticeStore.getInitialState(), true);
}
