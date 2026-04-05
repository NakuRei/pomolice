import { StrictMode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import App from "./App";
import { AppErrorFallback } from "./components/ErrorBoundary/AppErrorFallback";
import { FRONTEND_LOG_EVENT_IDS } from "./logging/logEvents";
import { createLogger } from "./logging/logger";

const logger = createLogger("error_boundary");

export function AppRoot(): React.ReactElement {
  return (
    <ErrorBoundary
      fallbackRender={(fallbackProps) => (
        <AppErrorFallback {...fallbackProps} />
      )}
      onError={(error, errorInfo): void => {
        logger.error(
          FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
          error,
          {
            boundary_scope: "app",
            component_stack: errorInfo.componentStack ?? undefined,
            failure_message: error.message,
          },
        );
      }}
      onReset={(): void => {
        logger.info(FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset, {
          boundary_scope: "app",
        });
      }}
    >
      <StrictMode>
        <App />
      </StrictMode>
    </ErrorBoundary>
  );
}
