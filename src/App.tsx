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
import { IconButton } from "./components/shared/IconButton";
import { InfoMessageBanner } from "./components/shared/InfoMessageBanner";
import { NotificationMuteToggleButton } from "./components/shared/NotificationMuteToggleButton";
import { useTimerSync } from "./hooks/useTimerSync";
import { useTimerStore } from "./store/timerStore";
import { useSettingsStore } from "./store/settingsStore";
import type { AppScreen } from "./types/appScreen";

const ESCAPE_BACK_TARGET_BY_SCREEN: Partial<Record<AppScreen, AppScreen>> = {
  settingsTop: "home",
  settingsTimer: "settingsTop",
  settingsAutoRun: "settingsTop",
  settingsNotification: "settingsTop",
};

const HEADER_STATUS_BADGE_BASE_CLASS_NAME =
  "inline-flex items-center rounded-full border px-2 py-1 text-xs font-semibold";
const HEADER_STATUS_BADGE_CLASS_NAME_BY_VARIANT = {
  success:
    `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-success-800 bg-success-950/40 text-success-200`,
  primary:
    `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-primary-800 bg-primary-950/40 text-primary-200`,
  warning:
    `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-warning-800 bg-warning-950/40 text-warning-200`,
} as const;

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
    <div className="flex min-h-screen flex-col space-y-8 bg-background p-8 text-on-background">
      {/* Backend connection error banner — only visible when connection fails */}
      <ConnectionBanner />
      <InfoMessageBanner />

      <div className="flex justify-between items-center">
        <div className="flex flex-col gap-2">
          <h1 className="text-3xl font-bold text-text">Pomolice</h1>
          <div
            className="flex items-center gap-2"
            data-testid="header-status-badges"
          >
            {autoRunFeatureEnabled && (
              <span
                className={HEADER_STATUS_BADGE_CLASS_NAME_BY_VARIANT.success}
                data-testid="header-badge-auto-run"
              >
                自動実行ON
              </span>
            )}
            {isAutoContinueEnabled && (
              <span
                className={HEADER_STATUS_BADGE_CLASS_NAME_BY_VARIANT.primary}
                data-testid="header-badge-auto-continue"
              >
                自動継続ON
              </span>
            )}
            {requiresManualContinue && (
              <span
                className={HEADER_STATUS_BADGE_CLASS_NAME_BY_VARIANT.warning}
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
          />

          <IconButton
            onClick={moveToSettingsTopScreen}
            label="設定を開く"
            testId="settings-button"
          >
            <SettingsGearIcon className="w-6 h-6" />
          </IconButton>
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
