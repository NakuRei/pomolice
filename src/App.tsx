import { useState } from 'react';
import {
  SettingsGearIcon,
  VolumeMutedIcon,
  VolumeOnIcon,
} from './components/icons';
import { TimerDisplay, TimerControls, SessionIndicator, CycleProgress } from './components/Timer';
import { ContinueDialog } from './components/ContinueDialog';
import { AutoStartConfirmDialog } from './components/AutoStartConfirmDialog';
import { SettingsModal } from './components/Settings';
import { FullscreenBreak } from './components/BreakScreen/FullscreenBreak';
import { ConnectionBanner } from './components/shared/ConnectionBanner';
import { InfoMessageBanner } from './components/shared/InfoMessageBanner';
import { useTimerSync } from './hooks/useTimerSync';
import { useTimerStore } from './store/timerStore';
import { useSettingsStore } from './store/settingsStore';

function App() {
  // Initialize timer sync with backend (single source of truth)
  useTimerSync();

  // Settings modal state
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  // Get fullscreen break state from store (updated by backend events)
  const isFullscreenBreak = useTimerStore((state) => state.isFullscreenBreak);
  const notificationSoundMuted = useSettingsStore((state) => state.notificationSoundMuted);
  const autoRunFeatureEnabled = useSettingsStore((state) => state.autoRunFeatureEnabled);
  const postBreakTransitionPolicy = useSettingsStore(
    (state) => state.post_break_transition_policy
  );
  const toggleNotificationSoundMuted = useSettingsStore(
    (state) => state.toggleNotificationSoundMuted
  );
  const isAutoContinueEnabled =
    postBreakTransitionPolicy === "start_next_work_automatically";

  // Show fullscreen break overlay when in break mode
  if (isFullscreenBreak) {
    return <FullscreenBreak />;
  }

  return (
    <div className="flex flex-col min-h-screen p-8 space-y-8">
      {/* Backend connection error banner — only visible when connection fails */}
      <ConnectionBanner />
      <InfoMessageBanner />

      <div className="flex justify-between items-center">
        <div className="flex flex-col gap-2">
          <h1 className="text-3xl font-bold text-gray-800">Pomolice</h1>
          {(autoRunFeatureEnabled || isAutoContinueEnabled) && (
            <div className="flex items-center gap-2" data-testid="header-status-badges">
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
            </div>
          )}
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={toggleNotificationSoundMuted}
            className={`transition-colors ${
              notificationSoundMuted
                ? "text-red-600 hover:text-red-700"
                : "text-gray-600 hover:text-gray-800"
            }`}
            title={notificationSoundMuted ? "通知音ミュート中" : "通知音オン"}
            data-testid="notification-mute-button"
            aria-label={notificationSoundMuted ? "通知音をオンにする" : "通知音をミュートする"}
          >
            {notificationSoundMuted ? (
              <VolumeMutedIcon className="w-6 h-6" />
            ) : (
              <VolumeOnIcon className="w-6 h-6" />
            )}
          </button>

          <button
            onClick={() => setIsSettingsOpen(true)}
            className="text-gray-600 hover:text-gray-800 transition-colors"
            title="設定"
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

      {/* Settings modal */}
      <SettingsModal isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />
    </div>
  );
}

export default App;
