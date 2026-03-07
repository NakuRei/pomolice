import { useEffect, useState } from "react";
import { SettingsGearIcon } from "./components/icons";
import {
  TimerDisplay,
  TimerControls,
  SessionIndicator,
  CycleProgress,
} from "./components/Timer";
import { ContinueDialog } from "./components/ContinueDialog";
import { AutoStartConfirmDialog } from "./components/AutoStartConfirmDialog";
import {
  SettingsAutoRunPage,
  SettingsNotificationPage,
  SettingsScreenLayout,
  SettingsTopPage,
  SettingsTimerPage,
} from "./components/Settings";
import { FullscreenBreak } from "./components/BreakScreen/FullscreenBreak";
import { ConnectionBanner } from "./components/shared/ConnectionBanner";
import { InfoMessageBanner } from "./components/shared/InfoMessageBanner";
import { NotificationMuteToggleButton } from "./components/shared/NotificationMuteToggleButton";
import { useTimerSync } from "./hooks/useTimerSync";
import { useTimerStore } from "./store/timerStore";
import { useSettingsStore } from "./store/settingsStore";
import type { AppScreen } from "./types/appScreen";

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
  const headerIconButtonClassName =
    "flex h-11 w-11 items-center justify-center rounded-full bg-transparent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500";

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
    if (currentScreen !== "settingsTop") {
      return;
    }

    const closeSettingsTopOnEscapeKey = (keyboardEvent: KeyboardEvent) => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen("home");
    };

    window.addEventListener("keydown", closeSettingsTopOnEscapeKey);
    return () => {
      window.removeEventListener("keydown", closeSettingsTopOnEscapeKey);
    };
  }, [currentScreen]);

  useEffect(() => {
    if (currentScreen !== "settingsTimer") {
      return;
    }

    const moveBackToSettingsTopOnEscapeKey = (keyboardEvent: KeyboardEvent) => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen("settingsTop");
    };

    window.addEventListener("keydown", moveBackToSettingsTopOnEscapeKey);
    return () => {
      window.removeEventListener("keydown", moveBackToSettingsTopOnEscapeKey);
    };
  }, [currentScreen]);

  useEffect(() => {
    if (currentScreen !== "settingsAutoRun") {
      return;
    }

    const moveBackToSettingsTopFromAutoRunScreen = (
      keyboardEvent: KeyboardEvent,
    ) => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen("settingsTop");
    };

    window.addEventListener("keydown", moveBackToSettingsTopFromAutoRunScreen);
    return () => {
      window.removeEventListener(
        "keydown",
        moveBackToSettingsTopFromAutoRunScreen,
      );
    };
  }, [currentScreen]);

  useEffect(() => {
    if (currentScreen !== "settingsNotification") {
      return;
    }

    const moveBackToSettingsTopFromNotificationScreen = (
      keyboardEvent: KeyboardEvent,
    ) => {
      if (keyboardEvent.key !== "Escape") {
        return;
      }

      keyboardEvent.preventDefault();
      setCurrentScreen("settingsTop");
    };

    window.addEventListener(
      "keydown",
      moveBackToSettingsTopFromNotificationScreen,
    );
    return () => {
      window.removeEventListener(
        "keydown",
        moveBackToSettingsTopFromNotificationScreen,
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
    <div className="flex flex-col min-h-screen p-8 space-y-8">
      {/* Backend connection error banner — only visible when connection fails */}
      <ConnectionBanner />
      <InfoMessageBanner />

      <div className="flex justify-between items-center">
        <div className="flex flex-col gap-2">
          <h1 className="text-3xl font-bold text-gray-800">Pomolice</h1>
          <div
            className="flex items-center gap-2"
            data-testid="header-status-badges"
          >
            {autoRunFeatureEnabled && (
              <span
                className="inline-flex items-center rounded-full border border-emerald-300 bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-800"
                data-testid="header-badge-auto-run"
              >
                自動実行ON
              </span>
            )}
            {isAutoContinueEnabled && (
              <span
                className="inline-flex items-center rounded-full border border-cyan-300 bg-cyan-50 px-2 py-1 text-xs font-semibold text-cyan-800"
                data-testid="header-badge-auto-continue"
              >
                自動継続ON
              </span>
            )}
            {requiresManualContinue && (
              <span
                className="inline-flex items-center rounded-full border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-800"
                data-testid="header-badge-require-manual-continue"
              >
                毎回確認
              </span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-3">
          <NotificationMuteToggleButton
            muted={notificationSoundMuted}
            onToggle={toggleNotificationSoundMuted}
            testId="notification-mute-button"
            iconClassName="h-6 w-6"
            className={headerIconButtonClassName}
          />

          <button
            onClick={moveToSettingsTopScreen}
            className={`${headerIconButtonClassName} text-gray-600 transition-colors hover:text-gray-800`}
            title="設定"
            aria-label="設定を開く"
            data-testid="settings-button"
          >
            <SettingsGearIcon className="w-6 h-6" />
          </button>
        </div>
      </div>

      {/* Session progress indicators */}
      <div className="flex flex-col items-center gap-3">
        <SessionIndicator />
        <CycleProgress />
      </div>

      <div className="flex-1 flex flex-col items-center justify-center">
        <TimerDisplay />
      </div>

      <TimerControls />

      {/* Continue dialog - shown after break completion */}
      <ContinueDialog />

      {/* Startup auto-start confirmation dialog */}
      <AutoStartConfirmDialog />
    </div>
  );
}

export default App;
