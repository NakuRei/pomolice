import { useEffect, useState } from "react";
import { AppShell } from "./AppShell";
import { FullscreenBreak } from "./components/BreakScreen/FullscreenBreak";
import { HomeScreen } from "./components/Home/HomeScreen";
import { SettingsScreenContent } from "./components/Settings/SettingsScreenContent";
import { useTimerSync } from "./hooks/useTimerSync";
import { useTimerStore } from "./store/timerStore";
import type { AppScreen, SettingsScreen } from "./types/appScreen";

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
  const settings = useTimerStore((state) => state.settings);
  const activeWindowSummary = useTimerStore(
    (state) => state.activeWindowSummary,
  );
  const toggleNotificationSoundMuted = useTimerStore(
    (state) => state.toggleNotificationSoundMuted,
  );
  const isAutoContinueEnabled =
    settings?.postBreakTransitionPolicy === "start_next_work_automatically";
  const requiresManualContinue =
    settings?.postBreakTransitionPolicy === "require_manual_continue";

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

  return (
    <AppShell
      currentScreen={currentScreen}
      isFullscreenBreak={isFullscreenBreak}
      onBackToHome={() => {
        moveToScreen("home");
      }}
      renderHomeScreen={() => (
        <HomeScreen
          notificationSoundMuted={settings?.notificationSoundMuted ?? null}
          isAutoRunWindowActive={activeWindowSummary !== null}
          isAutoContinueEnabled={isAutoContinueEnabled}
          requiresManualContinue={requiresManualContinue}
          onToggleNotificationSoundMuted={() => {
            void toggleNotificationSoundMuted();
          }}
          onOpenSettingsTopScreen={() => {
            moveToScreen("settingsTop");
          }}
        />
      )}
      renderFullscreenBreakScreen={() => <FullscreenBreak />}
      renderSettingsScreen={(settingsScreen: SettingsScreen) => (
        <SettingsScreenContent
          currentScreen={settingsScreen}
          moveToScreen={moveToScreen}
        />
      )}
    />
  );
}

export default App;
