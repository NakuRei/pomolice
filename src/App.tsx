import { useEffect, useState } from "react";
import { SettingsAutoRunPage } from "./components/Settings/SettingsAutoRunPage";
import { SettingsNotificationPage } from "./components/Settings/SettingsNotificationPage";
import { SettingsScreenLayout } from "./components/Settings/SettingsScreenLayout";
import { SettingsTopPage } from "./components/Settings/SettingsTopPage";
import { SettingsTimerPage } from "./components/Settings/SettingsTimerPage";
import { FullscreenBreak } from "./components/BreakScreen/FullscreenBreak";
import { HomeScreen } from "./components/Home/HomeScreen";
import { useTimerSync } from "./hooks/useTimerSync";
import { useSettingsStore } from "./store/settingsStore";
import { useTimerStore } from "./store/timerStore";
import type { AppScreen } from "./types/appScreen";

const ESCAPE_BACK_TARGET_BY_SCREEN: Partial<Record<AppScreen, AppScreen>> = {
  settingsTop: "home",
  settingsTimer: "settingsTop",
  settingsAutoRun: "settingsTop",
  settingsNotification: "settingsTop",
};

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
    (state) => state.post_break_transition_policy,
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
    return <FullscreenBreak />;
  }

  if (currentScreen === "settingsTop") {
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
  }

  if (currentScreen === "settingsTimer") {
    return (
      <SettingsScreenLayout mainTestId="settings-timer-main">
        <SettingsTimerPage
          openSettingsTopScreen={() => {
            moveToScreen("settingsTop");
          }}
        />
      </SettingsScreenLayout>
    );
  }

  if (currentScreen === "settingsAutoRun") {
    return (
      <SettingsScreenLayout mainTestId="settings-auto-run-main">
        <SettingsAutoRunPage
          openSettingsTopScreen={() => {
            moveToScreen("settingsTop");
          }}
        />
      </SettingsScreenLayout>
    );
  }

  if (currentScreen === "settingsNotification") {
    return (
      <SettingsScreenLayout mainTestId="settings-notification-main">
        <SettingsNotificationPage
          openSettingsTopScreen={() => {
            moveToScreen("settingsTop");
          }}
        />
      </SettingsScreenLayout>
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
