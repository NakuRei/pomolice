import { useEffect, useRef } from "react";
import { useTimerStore } from "../../store/timerStore";
import { DialogActionRow } from "../shared/DialogActionRow";
import { Button } from "../shared/Button";
import { DialogOverlay } from "../shared/DialogOverlay";
import { DialogPanel } from "../shared/DialogPanel";
import { InlineError } from "../shared/InlineError";

/**
 * ContinueDialog Component
 * Shows dialog after break completion asking if user wants to continue
 *
 * Phase 2: Displays after break session ends
 * - "続ける" button: Starts next work session
 * - "終了" button: Returns to idle state
 */
export function ContinueDialog() {
  const requiresManualContinueDecision = useTimerStore((state) => state.requiresManualContinueDecision);
  const completedPomodoros = useTimerStore((state) => state.completedPomodoros);
  const resolvePostBreakDecision = useTimerStore(
    (state) => state.resolvePostBreakDecision
  );
  const continueButtonReference = useRef<HTMLButtonElement>(null);

  const handleContinue = () => {
    // Errors are reported to errorStore by timerStore — InlineError handles display
    resolvePostBreakDecision("continue").catch(() => undefined);
  };

  const handleEnd = () => {
    // Errors are reported to errorStore by timerStore — InlineError handles display
    resolvePostBreakDecision("stop_cycle").catch(() => undefined);
  };

  useEffect(() => {
    if (!requiresManualContinueDecision) {
      return;
    }

    continueButtonReference.current?.focus();
  }, [requiresManualContinueDecision]);

  useEffect(() => {
    if (!requiresManualContinueDecision) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      event.preventDefault();
      handleEnd();
    };

    window.addEventListener("keydown", onKeyDown);

    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [requiresManualContinueDecision, resolvePostBreakDecision]);

  if (!requiresManualContinueDecision) {
    return null;
  }

  return (
    <DialogOverlay testId="continue-dialog">
      <DialogPanel labelledBy="continue-dialog-title">
        <h2
          id="continue-dialog-title"
          className="mb-4 text-2xl font-bold text-text"
        >
          1サイクル完了しました！
        </h2>
        <p className="mb-2 text-text-muted">
          お疲れ様でした。休憩が完了しました。
        </p>
        <p className="mb-6 text-text-muted">
          完了したポモドーロ: <span className="font-semibold">{completedPomodoros}</span>
        </p>
        <p className="mb-4 font-medium text-text">
          次のポモドーロを続けますか？
        </p>
        <p
          className="mb-4 rounded-md border border-warning-800 bg-warning-950/30 px-3 py-2 text-sm text-warning-200"
          data-testid="continue-dialog-manual-decision-message"
        >
          現在、手動判断待ちです。続けるか終了を選んでください。
        </p>
        {/* Error area: shown when manual decision command fails */}
        <div className="mb-4">
          <InlineError errorId="resolve_post_break_decision" />
        </div>
        <DialogActionRow>
          <Button
            appearance="secondary"
            size="md"
            onClick={handleEnd}
            className="flex-1"
            testId="continue-dialog-end"
          >
            終了
          </Button>
          <Button
            ref={continueButtonReference}
            appearance="primary"
            size="md"
            onClick={handleContinue}
            className="flex-1"
            testId="continue-dialog-continue"
          >
            続ける
          </Button>
        </DialogActionRow>
      </DialogPanel>
    </DialogOverlay>
  );
}
