import { useBackendObservationStore } from "@/backend/observationStore";
import { AutoStartConfirmDialog } from "./AutoStartConfirmDialog";
import { ContinueDialog } from "./ContinueDialog";
import { SettingsGearIcon } from "@/components/icons/AppIcons";
import { CycleProgress } from "./CycleProgress";
import { SessionDurationNotice } from "./SessionDurationNotice";
import { TimerControls } from "./TimerControls";
import { TimerNumber } from "./TimerNumber";
import { TimerPhaseLabel } from "./TimerPhaseLabel";
import { IconButton } from "@/components/shared/IconButton";
import { InfoMessageBanner } from "./InfoMessageBanner";
import { OperationNotice } from "@/components/shared/OperationNotice";
import { NotificationMuteToggleButton } from "@/features/settings/ui/NotificationMuteToggleButton";
import { SettingsFileNotice } from "@/features/settings/ui/SettingsFileNotice";
import { ToastStack } from "@/components/shared/ToastStack";

const HEADER_LINE_CLASS_NAME = "text-sm tracking-widest md:text-lg";
const SIDE_PADDING_CLASS_NAME = "px-8 md:px-20";
const CONTENT_WIDTH_CLASS_NAME = "mx-auto w-full max-w-4xl";

const SESSION_PHASE_LABEL = {
  work: "FOCUS",
  short_break: "BREAK",
  long_break: "LONG",
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
  isAutoRunWindowActive: boolean | null;
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
  const sessionType = useBackendObservationStore((state) => state.sessionType);
  const hasBootAutoStartOffer = useBackendObservationStore((state) =>
    Boolean(state.bootAutoStartOffer),
  );

  return (
    // Fills the space AppRoot leaves below its notices, rather than claiming
    // the whole viewport and pushing its own footer off-screen.
    <div className="flex h-full flex-col overflow-hidden bg-background bg-[radial-gradient(110%_65%_at_50%_0%,var(--color-primary-950)_0%,var(--color-background)_62%)] text-on-background">
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

            {isAutoRunWindowActive !== null && (
              <div
                className={`${HEADER_LINE_CLASS_NAME} ${
                  isAutoRunWindowActive
                    ? "text-success-300"
                    : "text-primary-600"
                }`}
                data-testid="header-auto-run"
              >
                {isAutoRunWindowActive ? "SCHEDULED" : "MANUAL"}
              </div>
            )}
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
            {sessionType !== null && (
              <TimerPhaseLabel
                sessionType={sessionType}
                className="tracking-[0.2em]"
              >
                {SESSION_PHASE_LABEL[sessionType]}
              </TimerPhaseLabel>
            )}
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
        <OperationNotice operation="start_work" />
        <OperationNotice operation="update_settings" />
        {!hasBootAutoStartOffer && (
          <OperationNotice
            operation="resolve_boot_auto_start_prompt"
            onRetry={null}
          />
        )}
      </ToastStack>

      <ContinueDialog />
      <AutoStartConfirmDialog />
    </div>
  );
}
