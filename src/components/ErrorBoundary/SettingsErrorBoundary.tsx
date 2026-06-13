import type { ReactNode } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { getFailureMessage } from "../../logging/createRootErrorCallbacks";
import { FRONTEND_LOG_EVENT_IDS } from "../../logging/logEvents";
import { createLogger } from "../../logging/logger";
import type { SettingsScreen } from "../../types/appScreen";
import { SettingsErrorFallback } from "./SettingsErrorFallback";

interface SettingsErrorBoundaryProps {
  children: ReactNode;
  currentScreen: SettingsScreen;
  onBackToHome: () => void;
}

const logger = createLogger("error_boundary");

export function SettingsErrorBoundary({
  children,
  currentScreen,
  onBackToHome,
}: SettingsErrorBoundaryProps): React.ReactElement {
  return (
    <ErrorBoundary
      fallbackRender={(fallbackProps) => (
        <SettingsErrorFallback {...fallbackProps} onBackToHome={onBackToHome} />
      )}
      onError={(error, errorInfo): void => {
        logger.error(
          FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
          error,
          {
            boundary_scope: "settings",
            component_stack: errorInfo.componentStack ?? undefined,
            current_screen: currentScreen,
            failure_message: getFailureMessage(error),
          },
        );
      }}
      onReset={(): void => {
        logger.info(FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset, {
          boundary_scope: "settings",
        });
      }}
      resetKeys={[currentScreen]}
    >
      {children}
    </ErrorBoundary>
  );
}
