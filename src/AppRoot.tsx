import { StrictMode } from "react";
import App from "./App";
import { AppErrorBoundary } from "./components/ErrorBoundary/AppErrorBoundary";

export function AppRoot(): React.ReactElement {
  return (
    <AppErrorBoundary>
      <StrictMode>
        <App />
      </StrictMode>
    </AppErrorBoundary>
  );
}
