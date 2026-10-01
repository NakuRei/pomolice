import { StrictMode } from "react";
import { AppSynchronization } from "./app/AppSynchronization";
import App from "./App";
import { AppErrorBoundary } from "@/app/ui/AppErrorBoundary";
import { TrayUnavailableNotice } from "@/app/ui/TrayUnavailableNotice";
import { InputMonitorFailureNotice } from "@/app/ui/InputMonitorFailureNotice";
import { EmergencyShortcutNotice } from "@/app/ui/EmergencyShortcutNotice";
import { TimerEffectsNotice } from "@/app/ui/TimerEffectsNotice";
import { ConnectionBanner } from "@/app/ui/ConnectionBanner";
import { OperationNotice } from "./components/shared/OperationNotice";

export function AppRoot(): React.ReactElement {
  return (
    <StrictMode>
      <AppSynchronization />
      <div className="flex h-screen flex-col">
        <ConnectionBanner />
        <TrayUnavailableNotice />
        <EmergencyShortcutNotice />
        <InputMonitorFailureNotice />
        <TimerEffectsNotice />
        <OperationNotice
          operation="retry_failed_timer_effects"
          onRetry={null}
        />
        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <AppErrorBoundary>
            <App />
          </AppErrorBoundary>
        </div>
      </div>
    </StrictMode>
  );
}
