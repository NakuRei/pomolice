import { AutoStartConfirmDialog } from "../AutoStartConfirmDialog";
import { ContinueDialog } from "../ContinueDialog";
import { SettingsGearIcon } from "../icons";
import {
  CycleProgress,
  SessionIndicator,
  TimerControls,
  TimerDisplay,
} from "../Timer";
import { ConnectionBanner } from "../shared/ConnectionBanner";
import { IconButton } from "../shared/IconButton";
import { InfoMessageBanner } from "../shared/InfoMessageBanner";
import { NotificationMuteToggleButton } from "../shared/NotificationMuteToggleButton";

const HEADER_STATUS_BADGE_BASE_CLASS_NAME =
  "inline-flex items-center rounded-full border px-2 py-1 text-xs font-semibold";
const HEADER_STATUS_BADGE_CLASS_NAME_BY_VARIANT = {
  success: `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-success-800 bg-success-950/40 text-success-200`,
  primary: `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-primary-800 bg-primary-950/40 text-primary-200`,
  warning: `${HEADER_STATUS_BADGE_BASE_CLASS_NAME} border-warning-800 bg-warning-950/40 text-warning-200`,
} as const;

/**
 * Fixed props for App's extracted home-screen branch.
 *
 * Usage example:
 * ```tsx
 * <HomeScreen
 *   notificationSoundMuted={notificationSoundMuted}
 *   autoRunFeatureEnabled={autoRunFeatureEnabled}
 *   isAutoContinueEnabled={isAutoContinueEnabled}
 *   requiresManualContinue={requiresManualContinue}
 *   onToggleNotificationSoundMuted={toggleNotificationSoundMuted}
 *   onOpenSettingsTopScreen={moveToSettingsTopScreen}
 * />
 * ```
 */
export interface HomeScreenProps {
  notificationSoundMuted: boolean;
  autoRunFeatureEnabled: boolean;
  isAutoContinueEnabled: boolean;
  requiresManualContinue: boolean;
  onToggleNotificationSoundMuted: () => void;
  onOpenSettingsTopScreen: () => void;
}

export function HomeScreen({
  notificationSoundMuted,
  autoRunFeatureEnabled,
  isAutoContinueEnabled,
  requiresManualContinue,
  onToggleNotificationSoundMuted,
  onOpenSettingsTopScreen,
}: HomeScreenProps): React.ReactElement {
  return (
    <div className="flex min-h-screen flex-col space-y-8 bg-background p-8 text-on-background">
      <ConnectionBanner />
      <InfoMessageBanner />

      <div className="flex items-center justify-between">
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
            onToggle={onToggleNotificationSoundMuted}
            testId="notification-mute-button"
            iconClassName="h-6 w-6"
          />

          <IconButton
            onClick={onOpenSettingsTopScreen}
            label="設定を開く"
            testId="settings-button"
          >
            <SettingsGearIcon className="h-6 w-6" />
          </IconButton>
        </div>
      </div>

      <div className="flex flex-col items-center gap-3">
        <SessionIndicator />
        <CycleProgress />
      </div>

      <div className="flex flex-1 flex-col items-center justify-center">
        <TimerDisplay />
      </div>

      <TimerControls />
      <ContinueDialog />
      <AutoStartConfirmDialog />
    </div>
  );
}
