import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { getFailureMessage } from "@/logging/createRootErrorCallbacks";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";
import { createLogger } from "@/logging/logger";
import { AppErrorFallback } from "./AppErrorFallback";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

const logger = createLogger("error_boundary");

export function AppErrorBoundary({
  children,
}: AppErrorBoundaryProps): React.ReactElement {
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
            failure_message: getFailureMessage(error),
          },
        );
      }}
      onReset={(): void => {
        logger.info(FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset, {
          boundary_scope: "app",
        });
      }}
    >
      {children}
    </ErrorBoundary>
  );
}
