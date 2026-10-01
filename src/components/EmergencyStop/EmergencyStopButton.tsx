import { useTimerObservationStore } from "@/store/timerObservationStore";
import { emergencyStop } from "@/timer/commands";
import { useEffect, useId, useRef, useState } from "react";

import { Button } from "../shared/Button";
import { DialogActionRow } from "../shared/DialogActionRow";
import { AppDialog } from "../shared/AppDialog";
import { OperationNotice } from "../shared/OperationNotice";

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
  const status = useTimerObservationStore((state) => state.status);

  const [showConfirmDialog, setShowConfirmDialog] = useState(false);
  const titleId = useId();
  const cancelButtonReference = useRef<HTMLButtonElement>(null);

  const isWorking = status === "working";

  const handleEmergencyStopClick = (): void => {
    setShowConfirmDialog(true);
  };

  const handleConfirm = (): void => {
    // Keep the dialog open on failure so the user can see the error and retry
    void emergencyStop().then((result) => {
      if (result.status === "accepted") setShowConfirmDialog(false);
    });
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
      <Button
        appearance="secondary"
        onClick={handleEmergencyStopClick}
        testId="emergency-stop-button"
        aria-label="緊急停止"
      >
        緊急停止
      </Button>

      {showConfirmDialog && (
        <AppDialog
          overlayTestId="emergency-stop-dialog"
          labelledBy={titleId}
          className="p-6"
        >
          <h2 id={titleId} className="mb-4 text-xl font-bold text-text">
            緊急停止しますか？
          </h2>
          <p className="mb-4 text-text-muted">
            現在のポモドーロセッションは記録されません。
            <br />
            緊急時以外は、セッション完了までお待ちください。
          </p>
          {/* Dialog stays open on failure; error shown here with retry button */}
          <div className="mb-4">
            <OperationNotice
              operation="emergency_stop"
              onRetry={handleConfirm}
            />
          </div>
          <DialogActionRow className="justify-end gap-3">
            <Button
              ref={cancelButtonReference}
              appearance="secondary"
              onClick={handleCancel}
              testId="emergency-stop-cancel"
            >
              キャンセル
            </Button>
            <Button
              appearance="danger"
              onClick={handleConfirm}
              testId="emergency-stop-confirm"
            >
              停止する
            </Button>
          </DialogActionRow>
        </AppDialog>
      )}
    </>
  );
}
