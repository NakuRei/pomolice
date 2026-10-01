/**
 * Confirmation Step Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Steps 1-4: Simple Yes/No confirmation dialogs
 * Each with progressively stronger warning messages
 */

import { useEffect, useRef } from "react";
import { getConfirmationMessage } from "@/features/break/skip/phraseGenerator";
import { Button } from "@/components/shared/Button";

interface ConfirmationStepProps {
  step: number; // 1-4
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmation step component for Yes/No dialogs
 */
export default function ConfirmationStep({
  step,
  onConfirm,
  onCancel,
}: ConfirmationStepProps): React.ReactElement {
  const cancelButtonReference = useRef<HTMLButtonElement>(null);
  const message = getConfirmationMessage(step);

  useEffect(() => {
    cancelButtonReference.current?.focus();
  }, [step]);

  // Get warning level for styling (higher step = stronger warning)
  const getWarningStyle = ():
    | "text-danger-400 border-danger-500"
    | "text-warning-400 border-warning-500"
    | "text-warning-300 border-warning-400"
    | "text-primary-500 border-primary-600" => {
    if (step === 4) {
      return "text-danger-400 border-danger-500";
    } else if (step === 3) {
      return "text-warning-400 border-warning-500";
    } else if (step === 2) {
      return "text-warning-300 border-warning-400";
    } else {
      return "text-primary-500 border-primary-600";
    }
  };

  return (
    <div className="space-y-8">
      {/* Warning message */}
      <div
        className={`rounded-lg border-2 p-6 text-center ${getWarningStyle()}`}
      >
        <p className="text-xl font-semibold">{message}</p>
      </div>

      {/* Health reminder */}
      {step >= 2 && (
        <div className="rounded-lg bg-surface-raised p-4 text-center text-sm text-text-muted">
          <p>
            💡 Reminder: Regular breaks improve focus, productivity, and health.
          </p>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex justify-center space-x-4">
        <Button
          ref={cancelButtonReference}
          appearance="primary"
          onClick={onCancel}
        >
          No, Take Break
        </Button>
        <Button appearance="secondary" onClick={onConfirm}>
          Yes, Continue
        </Button>
      </div>

      {/* Skip hint */}
      <div className="text-center text-xs text-text-muted">
        <p>Pressing any button will reset your break timer</p>
      </div>
    </div>
  );
}
