import { StrictMode } from "react";
import App from "./App";
import { AppErrorBoundary } from "./components/ErrorBoundary/AppErrorBoundary";
import { TrayUnavailableNotice } from "./components/shared/TrayUnavailableNotice";
import { InputMonitorFailureNotice } from "./components/shared/InputMonitorFailureNotice";
import { EmergencyShortcutNotice } from "./components/shared/EmergencyShortcutNotice";
import { TimerEffectsNotice } from "./components/shared/TimerEffectsNotice";
import { ConnectionBanner } from "./components/shared/ConnectionBanner";
import { OperationNotice } from "./components/shared/OperationNotice";

export function AppRoot(): React.ReactElement {
  return (
    <StrictMode>
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
