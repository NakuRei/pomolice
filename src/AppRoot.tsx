import { StrictMode } from "react";
import App from "./App";
import { AppErrorBoundary } from "./components/ErrorBoundary/AppErrorBoundary";
import { TrayUnavailableNotice } from "./components/shared/TrayUnavailableNotice";
import { InputMonitorFailureNotice } from "./components/shared/InputMonitorFailureNotice";
import { EmergencyShortcutNotice } from "./components/shared/EmergencyShortcutNotice";
import { TimerEffectsNotice } from "./components/shared/TimerEffectsNotice";

export function AppRoot(): React.ReactElement {
  return (
    <StrictMode>
      <div className="flex h-screen flex-col">
        <TrayUnavailableNotice />
        <EmergencyShortcutNotice />
        <InputMonitorFailureNotice />
        <TimerEffectsNotice />
        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <AppErrorBoundary>
            <App />
          </AppErrorBoundary>
        </div>
      </div>
    </StrictMode>
  );
}
