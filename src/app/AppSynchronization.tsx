import { useBackendSync } from "./sync/useBackendSync";
import { useEmergencyShortcutSync } from "./sync/useEmergencyShortcutSync";
import { useInputMonitorSync } from "./sync/useInputMonitorSync";
import { useTraySync } from "./sync/useTraySync";

export function AppSynchronization(): null {
  useBackendSync();
  useEmergencyShortcutSync();
  useInputMonitorSync();
  useTraySync();
  return null;
}
