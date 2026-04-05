import { useEffect, useState } from "react";
import { ErrorBoundary } from "react-error-boundary";
import { FullscreenBreakErrorFallback } from "./components/ErrorBoundary/FullscreenBreakErrorFallback";
import { SettingsErrorFallback } from "./components/ErrorBoundary/SettingsErrorFallback";
import { SettingsAutoRunPage } from "./components/Settings/SettingsAutoRunPage";
import { SettingsNotificationPage } from "./components/Settings/SettingsNotificationPage";
import { SettingsScreenLayout } from "./components/Settings/SettingsScreenLayout";
import { SettingsTopPage } from "./components/Settings/SettingsTopPage";
import { SettingsTimerPage } from "./components/Settings/SettingsTimerPage";
import { FullscreenBreak } from "./components/BreakScreen/FullscreenBreak";
import { HomeScreen } from "./components/Home/HomeScreen";
import { useTimerSync } from "./hooks/useTimerSync";
import { FRONTEND_LOG_EVENT_IDS } from "./logging/logEvents";
import { createLogger } from "./logging/logger";
import { useSettingsStore } from "./store/settingsStore";
import { useTimerStore } from "./store/timerStore";
import type { AppScreen } from "./types/appScreen";

const ESCAPE_BACK_TARGET_BY_SCREEN: Partial<Record<AppScreen, AppScreen>> = {
  settingsTop: "home",
  settingsTimer: "settingsTop",
  settingsAutoRun: "settingsTop",
  settingsNotification: "settingsTop",
};

const SETTINGS_SCREEN_VALUES = [
  "settingsTop",
  "settingsTimer",
  "settingsAutoRun",
  "settingsNotification",
] as const satisfies readonly AppScreen[];

type SettingsScreen = (typeof SETTINGS_SCREEN_VALUES)[number];

const logger = createLogger("error_boundary");

function isSettingsScreen(screen: AppScreen): screen is SettingsScreen {
  return SETTINGS_SCREEN_VALUES.includes(screen as SettingsScreen);
}

function assertNeverSettingsScreen(screen: never): never {
  throw new Error(`Unknown settings screen: ${String(screen)}`);
}

function renderSettingsScreen(
  currentScreen: SettingsScreen,
  moveToScreen: (nextScreen: AppScreen) => void,
): React.ReactElement {
  switch (currentScreen) {
    case "settingsTop":
      return (
        <SettingsScreenLayout mainTestId="settings-top-main">
          <SettingsTopPage
            openHomeScreen={() => {
              moveToScreen("home");
            }}
            openTimerSettings={() => {
              moveToScreen("settingsTimer");
            }}
            openAutoRunSettings={() => {
              moveToScreen("settingsAutoRun");
            }}
            openNotificationSettings={() => {
              moveToScreen("settingsNotification");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsTimer":
      return (
        <SettingsScreenLayout mainTestId="settings-timer-main">
          <SettingsTimerPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsAutoRun":
      return (
        <SettingsScreenLayout mainTestId="settings-auto-run-main">
          <SettingsAutoRunPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsNotification":
      return (
        <SettingsScreenLayout mainTestId="settings-notification-main">
          <SettingsNotificationPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    default:
      return assertNeverSettingsScreen(currentScreen);
  }
}

function App(): React.ReactElement {
  // Initialize timer sync with backend (single source of truth)
  useTimerSync();

  const [currentScreen, setCurrentScreen] = useState<AppScreen>("home");
  // Get fullscreen break state from store (updated by backend events)
  const isFullscreenBreak = useTimerStore((state) => state.isFullscreenBreak);
  const [previousFullscreenBreakState, setPreviousFullscreenBreakState] =
    useState(isFullscreenBreak);
  const notificationSoundMuted = useSettingsStore(
    (state) => state.notificationSoundMuted,
  );
  const autoRunFeatureEnabled = useSettingsStore(
    (state) => state.autoRunFeatureEnabled,
  );
  const postBreakTransitionPolicy = useSettingsStore(
    (state) => state.postBreakTransitionPolicy,
  );
  const toggleNotificationSoundMuted = useSettingsStore(
    (state) => state.toggleNotificationSoundMuted,
  );
  const isAutoContinueEnabled =
    postBreakTransitionPolicy === "start_next_work_automatically";
  const requiresManualContinue =
    postBreakTransitionPolicy === "require_manual_continue";

  const moveToScreen = (nextScreen: AppScreen): void => {
    setCurrentScreen(nextScreen);
  };

  if (isFullscreenBreak !== previousFullscreenBreakState) {
    setPreviousFullscreenBreakState(isFullscreenBreak);
    if (isFullscreenBreak && currentScreen !== "home") {
      setCurrentScreen("home");
    }
  }

  useEffect(() => {
    const escapeBackTarget = ESCAPE_BACK_TARGET_BY_SCREEN[currentScreen];
    if (!escapeBackTarget) {
      return;
    }

    const moveBackOnEscapeKey = (keyboardEvent: KeyboardEvent): void => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen(escapeBackTarget);
    };

    window.addEventListener("keydown", moveBackOnEscapeKey);
    return (): void => {
      window.removeEventListener("keydown", moveBackOnEscapeKey);
    };
  }, [currentScreen]);

  // Show fullscreen break overlay when in break mode
  if (isFullscreenBreak) {
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
              failure_message: error.message,
            },
          );
        }}
        onReset={(): void => {
          logger.info(FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset, {
            boundary_scope: "fullscreen_break",
          });
        }}
      >
        <FullscreenBreak />
      </ErrorBoundary>
    );
  }

  if (isSettingsScreen(currentScreen)) {
    return (
      <ErrorBoundary
        fallbackRender={(fallbackProps) => (
          <SettingsErrorFallback
            {...fallbackProps}
            onBackToHome={() => {
              moveToScreen("home");
            }}
          />
        )}
        onError={(error, errorInfo): void => {
          logger.error(
            FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
            error,
            {
              boundary_scope: "settings",
              component_stack: errorInfo.componentStack ?? undefined,
              current_screen: currentScreen,
              failure_message: error.message,
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
        {renderSettingsScreen(currentScreen, moveToScreen)}
      </ErrorBoundary>
    );
  }

  return (
    <HomeScreen
      notificationSoundMuted={notificationSoundMuted}
      autoRunFeatureEnabled={autoRunFeatureEnabled}
      isAutoContinueEnabled={isAutoContinueEnabled}
      requiresManualContinue={requiresManualContinue}
      onToggleNotificationSoundMuted={toggleNotificationSoundMuted}
      onOpenSettingsTopScreen={() => {
        moveToScreen("settingsTop");
      }}
    />
  );
}

export default App;
