import { AutoStartConfirmDialog } from "../AutoStartConfirmDialog";
import { ContinueDialog } from "@/components/ContinueDialog/ContinueDialog";
import { SettingsGearIcon } from "@/components/icons/AppIcons";
import { CycleProgress } from "@/components/Timer/CycleProgress";
import { SessionDurationNotice } from "@/components/Timer/SessionDurationNotice";
import { TimerControls } from "@/components/Timer/TimerControls";
import { TimerNumber } from "@/components/Timer/TimerNumber";
import { TimerPhaseLabel } from "@/components/Timer/TimerPhaseLabel";
import { ConnectionBanner } from "../shared/ConnectionBanner";
import { IconButton } from "../shared/IconButton";
import { InfoMessageBanner } from "../shared/InfoMessageBanner";
import { InlineError } from "../shared/InlineError";
import { NotificationMuteToggleButton } from "../shared/NotificationMuteToggleButton";
import { SettingsFileNotice } from "../shared/SettingsFileNotice";
import { ToastStack } from "../shared/ToastStack";
import { useTimerStore } from "@/store/timerStore";
import { SessionType } from "@/types/timer";

const HEADER_LINE_CLASS_NAME = "text-sm tracking-widest md:text-lg";
const SIDE_PADDING_CLASS_NAME = "px-8 md:px-20";
const CONTENT_WIDTH_CLASS_NAME = "mx-auto w-full max-w-4xl";

const SESSION_PHASE_LABEL = {
  [SessionType.WORK]: "FOCUS",
  [SessionType.SHORT_BREAK]: "BREAK",
  [SessionType.LONG_BREAK]: "LONG",
} as const;

/**
 * Fixed props for App's extracted home-screen branch.
 *
 * Usage example:
 * ```tsx
 * <HomeScreen
 *   notificationSoundMuted={notificationSoundMuted}
 *   isAutoRunWindowActive={activeWindowSummary !== null}
 *   isAutoContinueEnabled={isAutoContinueEnabled}
 *   requiresManualContinue={requiresManualContinue}
 *   onToggleNotificationSoundMuted={toggleNotificationSoundMuted}
 *   onOpenSettingsTopScreen={moveToSettingsTopScreen}
 * />
 * ```
 */
export interface HomeScreenProps {
  /** Null until the backend has sent the settings; the mute toggle is hidden until then. */
  notificationSoundMuted: boolean | null;
  isAutoRunWindowActive: boolean;
  isAutoContinueEnabled: boolean;
  requiresManualContinue: boolean;
  onToggleNotificationSoundMuted: () => void;
  onOpenSettingsTopScreen: () => void;
}

export function HomeScreen({
  notificationSoundMuted,
  isAutoRunWindowActive,
  isAutoContinueEnabled,
  requiresManualContinue,
  onToggleNotificationSoundMuted,
  onOpenSettingsTopScreen,
}: HomeScreenProps): React.ReactElement {
  const sessionType = useTimerStore((state) => state.sessionType);
  const hasBootAutoStartOffer = useTimerStore((state) =>
    Boolean(state.bootAutoStartOffer),
  );

  return (
    // Fills the space AppRoot leaves below its notices, rather than claiming
    // the whole viewport and pushing its own footer off-screen.
    <div className="flex h-full flex-col overflow-hidden bg-background bg-[radial-gradient(110%_65%_at_50%_0%,var(--color-primary-950)_0%,var(--color-background)_62%)] text-on-background">
      <ConnectionBanner />
      <SettingsFileNotice />

      <div
        className={`grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)] grid-rows-[minmax(min-content,1fr)_auto_minmax(min-content,1fr)] gap-y-1 py-4 md:py-6 ${SIDE_PADDING_CLASS_NAME}`}
      >
        <div
          className={`flex items-center justify-between self-start ${CONTENT_WIDTH_CLASS_NAME}`}
        >
          <div className="flex min-w-0 flex-col gap-1 md:flex-row md:items-center md:gap-5">
            {requiresManualContinue && (
              <div
                className={`${HEADER_LINE_CLASS_NAME} text-warning-200`}
                data-testid="header-mode-ask-each-time"
              >
                ASK_EACH_TIME
              </div>
            )}
            {isAutoContinueEnabled && (
              <div
                className={`${HEADER_LINE_CLASS_NAME} text-primary-300`}
                data-testid="header-mode-auto-continue"
              >
                AUTO_CONTINUE
              </div>
            )}

            <div
              className={`${HEADER_LINE_CLASS_NAME} ${
                isAutoRunWindowActive ? "text-success-300" : "text-primary-600"
              }`}
              data-testid="header-auto-run"
            >
              {isAutoRunWindowActive ? "SCHEDULED" : "MANUAL"}
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            {notificationSoundMuted !== null && (
              <NotificationMuteToggleButton
                muted={notificationSoundMuted}
                onToggle={onToggleNotificationSoundMuted}
              />
            )}

            <IconButton
              onClick={onOpenSettingsTopScreen}
              label="設定を開く"
              testId="settings-button"
            >
              <SettingsGearIcon className="size-5" />
            </IconButton>
          </div>
        </div>

        <div
          className={`grid gap-y-[clamp(0.625rem,3vh,1.25rem)] ${CONTENT_WIDTH_CLASS_NAME}`}
        >
          <div className="flex items-baseline justify-between gap-4">
            <TimerNumber />
            <TimerPhaseLabel
              sessionType={sessionType}
              className="tracking-[0.2em]"
            >
              {SESSION_PHASE_LABEL[sessionType]}
            </TimerPhaseLabel>
          </div>
          <CycleProgress />
          <SessionDurationNotice />
        </div>

        <div
          className={`flex justify-end self-end ${CONTENT_WIDTH_CLASS_NAME}`}
        >
          <TimerControls />
        </div>
      </div>

      <ToastStack>
        <InfoMessageBanner />
        {/* Shown when start_work fails; auto-clears after 5 seconds */}
        <InlineError errorId="start_work" />
        <InlineError errorId="update_settings" />
        {!hasBootAutoStartOffer && (
          <InlineError
            errorId="resolve_boot_auto_start_prompt"
            onRetry={null}
          />
        )}
      </ToastStack>

      <ContinueDialog />
      <AutoStartConfirmDialog />
    </div>
  );
}
