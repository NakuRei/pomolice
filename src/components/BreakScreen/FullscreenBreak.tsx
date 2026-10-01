import { useTimerObservationStore } from "@/store/timerObservationStore";
import { emergencyStop, setAwayDuringBreak } from "@/timer/commands";
import { toggleNotificationSoundMuted } from "@/features/settings/commands";

import { useSkipFlowStore } from "@/store/skipFlowStore";
import { FootprintsIcon, SkipForwardIcon } from "@/components/icons/AppIcons";
import { CycleProgress } from "@/components/Timer/CycleProgress";
import { TimerNumber } from "@/components/Timer/TimerNumber";
import { TimerPhaseLabel } from "@/components/Timer/TimerPhaseLabel";
import SkipFlowModal from "../SkipFlow/SkipFlowModal";
import { Button } from "../shared/Button";
import { OperationNotice } from "../shared/OperationNotice";
import { NotificationMuteToggleButton } from "../shared/NotificationMuteToggleButton";
import { EmergencyShortcutHint } from "../shared/EmergencyShortcutNotice";

const SIDE_PADDING_CLASS_NAME = "px-8 md:px-24";

/**
 * Input monitoring and penalties are handled entirely by the backend; this
 * component only displays the break state and offers the skip flow.
 */
export function FullscreenBreak(): React.ReactElement {
  const sessionType = useTimerObservationStore((state) => state.sessionType);
  const settings = useTimerObservationStore((state) => state.settings);

  const isAwayDuringBreak = useTimerObservationStore(
    (state) => state.isAwayDuringBreak,
  );

  const isActive = useSkipFlowStore((state) => state.isActive);
  const startSkipFlow = useSkipFlowStore((state) => state.startSkipFlow);
  const resetSkipFlow = useSkipFlowStore((state) => state.resetSkipFlow);

  const handleSkipClick = (): void => {
    startSkipFlow();
  };

  const handleSkipComplete = (): void => {
    void emergencyStop();
    resetSkipFlow();
  };

  const handleSkipCancel = (): void => {
    resetSkipFlow();
  };

  // Being away only suspends the automatic start of the next work session, so
  // an unattended machine does not run through work sessions nobody is doing.
  // The break itself keeps running and stays locked.
  const offersAway = useTimerObservationStore(
    (state) =>
      state.settings?.postBreakTransitionPolicy ===
        "start_next_work_automatically" && state.isAwayDuringBreak !== null,
  );

  const handleAwayToggle = (): void => {
    if (isAwayDuringBreak !== null) void setAwayDuringBreak(!isAwayDuringBreak);
  };

  return (
    <div className="absolute inset-0 z-50 flex flex-col overflow-hidden bg-break-base bg-[radial-gradient(70%_80%_at_50%_100%,var(--color-break-glow)_0%,var(--color-break-mid)_55%,var(--color-break-base)_100%)] text-on-background accent-break">
      <div
        className={`flex shrink-0 items-center justify-end pt-6 md:pt-7 ${SIDE_PADDING_CLASS_NAME}`}
      >
        {settings !== null && (
          <NotificationMuteToggleButton
            muted={settings.notificationSoundMuted}
            onToggle={() => {
              void toggleNotificationSoundMuted();
            }}
          />
        )}
      </div>
      <div className={SIDE_PADDING_CLASS_NAME}>
        <OperationNotice operation="update_settings" />
      </div>

      <div
        className={`flex min-h-0 flex-1 flex-col items-center justify-center gap-15 md:gap-16.5 ${SIDE_PADDING_CLASS_NAME}`}
      >
        <div className="flex flex-col items-center gap-11.25 md:gap-12.5">
          {sessionType !== null && (
            <TimerPhaseLabel
              sessionType={sessionType}
              className="tracking-[0.3em]"
            >
              {sessionType === "long_break" ? "LONG BREAK" : "BREAK"}
            </TimerPhaseLabel>
          )}
          <TimerNumber />
        </div>

        <div className="w-full max-w-140">
          <CycleProgress />
        </div>
      </div>

      <div
        className={`grid shrink-0 grid-cols-[1fr_auto_1fr] items-end pb-8 md:pb-13 ${SIDE_PADDING_CLASS_NAME}`}
      >
        <div className="justify-self-start">
          <EmergencyShortcutHint />
        </div>
        <div className="flex flex-col items-center gap-4">
          {offersAway && isAwayDuringBreak === true && (
            <p role="status" className="text-sm text-text-muted">
              離席中：休憩後は自動で作業を開始しません
            </p>
          )}
          <div className="flex items-center gap-3.5">
            <Button
              appearance="outline"
              onClick={handleSkipClick}
              aria-label="休憩をスキップ"
            >
              <SkipForwardIcon className="size-5" />
              スキップ
            </Button>
            {offersAway && (
              <Button
                appearance="primary"
                onClick={handleAwayToggle}
                className="shadow-2xl shadow-accent-glow"
              >
                {isAwayDuringBreak === true ? (
                  "戻りました"
                ) : (
                  <>
                    <FootprintsIcon className="size-5" />
                    離席する
                  </>
                )}
              </Button>
            )}
          </div>
        </div>
      </div>

      {isActive && (
        <SkipFlowModal
          onComplete={handleSkipComplete}
          onCancel={handleSkipCancel}
        />
      )}
    </div>
  );
}
