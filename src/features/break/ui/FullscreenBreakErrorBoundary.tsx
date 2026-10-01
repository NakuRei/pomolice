import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { getFailureMessage } from "@/logging/createRootErrorCallbacks";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";
import { createLogger } from "@/logging/logger";
import { FullscreenBreakErrorFallback } from "./FullscreenBreakErrorFallback";

interface FullscreenBreakErrorBoundaryProps {
  children: ReactNode;
}

const logger = createLogger("error_boundary");

export function FullscreenBreakErrorBoundary({
  children,
}: FullscreenBreakErrorBoundaryProps): React.ReactElement {
  return (
    <ErrorBoundary
      fallbackRender={(fallbackProps) => (
        <FullscreenBreakErrorFallback {...fallbackProps} />
      )}
      onError={(error, errorInfo): void => {
        logger.error(
          FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
          error,
          {
            boundary_scope: "fullscreen_break",
            component_stack: errorInfo.componentStack ?? undefined,
            failure_message: getFailureMessage(error),
          },
        );
      }}
      onReset={(): void => {
        logger.info(FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset, {
          boundary_scope: "fullscreen_break",
        });
      }}
    >
      {children}
    </ErrorBoundary>
  );
}
