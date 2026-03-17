import { useEffect, useState } from "react";
import {
  SettingsAutoRunPage,
  SettingsNotificationPage,
  SettingsScreenLayout,
  SettingsTopPage,
  SettingsTimerPage,
} from "./components/Settings";
import { FullscreenBreak } from "./components/BreakScreen/FullscreenBreak";
import { HomeScreen } from "./components/Home";
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

function App() {
  // Initialize timer sync with backend (single source of truth)
  useTimerSync();

  const [currentScreen, setCurrentScreen] = useState<AppScreen>("home");

  // Get fullscreen break state from store (updated by backend events)
  const isFullscreenBreak = useTimerStore((state) => state.isFullscreenBreak);
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

  const moveToSettingsTopScreen = () => {
    setCurrentScreen("settingsTop");
  };
  const moveToSettingsTimerScreen = () => {
    setCurrentScreen("settingsTimer");
  };
  const moveToSettingsAutoRunScreen = () => {
    setCurrentScreen("settingsAutoRun");
  };
  const moveToSettingsNotificationScreen = () => {
    setCurrentScreen("settingsNotification");
  };
  const moveToHomeScreen = () => {
    setCurrentScreen("home");
  };

  useEffect(() => {
    if (!isFullscreenBreak) {
      return;
    }

    setCurrentScreen("home");
  }, [isFullscreenBreak]);

  useEffect(() => {
    const escapeBackTarget = ESCAPE_BACK_TARGET_BY_SCREEN[currentScreen];
    if (!escapeBackTarget) {
      return;
    }

    const moveBackOnEscapeKey = (keyboardEvent: KeyboardEvent) => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen(escapeBackTarget);
    };

    window.addEventListener("keydown", moveBackOnEscapeKey);
    return () => {
      window.removeEventListener(
        "keydown",
        moveBackOnEscapeKey,
      );
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
          openHomeScreen={moveToHomeScreen}
          openTimerSettings={moveToSettingsTimerScreen}
          openAutoRunSettings={moveToSettingsAutoRunScreen}
          openNotificationSettings={moveToSettingsNotificationScreen}
        />
      </SettingsScreenLayout>
    );
  }

  if (currentScreen === "settingsTimer") {
    return (
      <SettingsScreenLayout mainTestId="settings-timer-main">
        <SettingsTimerPage openSettingsTopScreen={moveToSettingsTopScreen} />
      </SettingsScreenLayout>
    );
  }

  if (currentScreen === "settingsAutoRun") {
    return (
      <SettingsScreenLayout mainTestId="settings-auto-run-main">
        <SettingsAutoRunPage openSettingsTopScreen={moveToSettingsTopScreen} />
      </SettingsScreenLayout>
    );
  }

  if (currentScreen === "settingsNotification") {
    return (
      <SettingsScreenLayout mainTestId="settings-notification-main">
        <SettingsNotificationPage
          openSettingsTopScreen={moveToSettingsTopScreen}
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
      onOpenSettingsTopScreen={moveToSettingsTopScreen}
    />
  );
}

export default App;
