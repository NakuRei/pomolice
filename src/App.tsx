import {
  useBackendObservationStore,
  selectBreakLockObservation,
} from "@/backend/observationStore";
import { toggleNotificationSoundMuted } from "@/features/settings/commands";
import { useEffect, useState } from "react";
import { AppShell } from "./AppShell";
import { FullscreenBreak } from "@/features/break/ui/FullscreenBreak";
import { HomeScreen } from "@/features/timer/ui/HomeScreen";
import { SettingsScreenContent } from "./features/settings/ui/SettingsScreenContent";
import type { AppScreen, SettingsScreen } from "./types/appScreen";

const ESCAPE_BACK_TARGET_BY_SCREEN: Partial<Record<AppScreen, AppScreen>> = {
  settingsTop: "home",
  settingsTimer: "settingsTop",
  settingsAutoRun: "settingsTop",
  settingsNotification: "settingsTop",
};

function App(): React.ReactElement {
  const [currentScreen, setCurrentScreen] = useState<AppScreen>("home");
  // Get fullscreen break state from store (updated by backend events)
  const isFullscreenBreak =
    useBackendObservationStore(selectBreakLockObservation) === "locked";
  const [previousFullscreenBreakState, setPreviousFullscreenBreakState] =
    useState(isFullscreenBreak);
  const settings = useBackendObservationStore((state) => state.settings);
  const activeWindowSummary = useBackendObservationStore(
    (state) => state.activeWindowSummary,
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
          isAutoRunWindowActive={
            activeWindowSummary === undefined
              ? null
              : activeWindowSummary !== null
          }
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
