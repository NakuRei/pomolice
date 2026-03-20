import { useEffect, useRef, useState } from "react";
import { useTimerStore, TimerStatus } from "../../store/timerStore";
import { Button } from "../shared/Button";
import { DialogActionRow } from "../shared/DialogActionRow";
import { DialogOverlay } from "../shared/DialogOverlay";
import { DialogPanel } from "../shared/DialogPanel";
import { InlineError } from "../shared/InlineError";

/**
 * EmergencyStopButton Component
 * Provides emergency stop functionality with confirmation dialog
 *
 * Phase 2: Emergency stop is available at any time during timer execution
 * - Shows confirmation dialog before stopping
 * - Current session is invalidated (not recorded in statistics)
 * - No penalty applied (emergency situations are respected)
 */
export function EmergencyStopButton(): React.ReactElement | null {
  const status = useTimerStore((state) => state.status);
  const emergencyStop = useTimerStore((state) => state.emergencyStop);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);
  const cancelButtonReference = useRef<HTMLButtonElement>(null);

  const isWorking = status === TimerStatus.WORKING;

  const handleEmergencyStopClick = (): void => {
    setShowConfirmDialog(true);
  };

  const handleConfirm = (): void => {
    // Keep the dialog open on failure so the user can see the error and retry
    emergencyStop()
      .then(() => {
        setShowConfirmDialog(false);
      })
      .catch(() => undefined);
  };

  const handleCancel = (): void => {
    setShowConfirmDialog(false);
  };

  useEffect(() => {
    if (!showConfirmDialog || !isWorking) {
      return;
    }

    cancelButtonReference.current?.focus();
  }, [isWorking, showConfirmDialog]);

  useEffect(() => {
    if (!showConfirmDialog || !isWorking) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== "Escape") {
        return;
      }

      event.preventDefault();
      setShowConfirmDialog(false);
    };

    window.addEventListener("keydown", onKeyDown);

    return (): void => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isWorking, showConfirmDialog]);

  // Don't show button if timer is not working
  if (!isWorking) {
    return null;
  }

  return (
    <>
      <button
        onClick={handleEmergencyStopClick}
        className="text-xs text-text-muted underline transition-colors hover:text-danger-400"
        data-testid="emergency-stop-button"
        aria-label="緊急停止"
      >
        緊急停止
      </button>

      {showConfirmDialog && (
        <DialogOverlay testId="emergency-stop-dialog">
          <DialogPanel labelledBy="emergency-stop-dialog-title" className="p-6">
            <h2
              id="emergency-stop-dialog-title"
              className="mb-4 text-xl font-bold text-text"
            >
              緊急停止しますか？
            </h2>
            <p className="mb-4 text-text-muted">
              現在のポモドーロセッションは記録されません。
              <br />
              緊急時以外は、セッション完了までお待ちください。
            </p>
            {/* Dialog stays open on failure; error shown here with retry button */}
            <div className="mb-4">
              <InlineError errorId="emergency_stop" onRetry={handleConfirm} />
            </div>
            <DialogActionRow className="justify-end gap-3">
              <Button
                ref={cancelButtonReference}
                appearance="secondary"
                size="sm"
                onClick={handleCancel}
                testId="emergency-stop-cancel"
              >
                キャンセル
              </Button>
              <Button
                appearance="danger"
                size="sm"
                onClick={handleConfirm}
                testId="emergency-stop-confirm"
              >
                停止する
              </Button>
            </DialogActionRow>
          </DialogPanel>
        </DialogOverlay>
      )}
    </>
  );
}
