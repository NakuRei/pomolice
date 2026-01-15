/**
 * Confirmation Step Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Steps 1-4: Simple Yes/No confirmation dialogs
 * Each with progressively stronger warning messages
 */

import { getConfirmationMessage } from "../../../utils/phraseGenerator";

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
}: ConfirmationStepProps) {
  const message = getConfirmationMessage(step);

  // Get warning level for styling (higher step = stronger warning)
  const getWarningStyle = () => {
    if (step === 4) {
      return "text-red-400 border-red-500";
    } else if (step === 3) {
      return "text-orange-400 border-orange-500";
    } else if (step === 2) {
      return "text-yellow-400 border-yellow-500";
    } else {
      return "text-blue-400 border-blue-500";
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
        <div className="rounded-lg bg-gray-800 p-4 text-center text-sm text-gray-300">
          <p>
            💡 Reminder: Regular breaks improve focus, productivity, and health.
          </p>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex justify-center space-x-4">
        <button
          onClick={onCancel}
          className="rounded-lg bg-green-600 px-8 py-3 font-semibold text-white transition-colors hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-2 focus:ring-offset-gray-900"
        >
          No, Take Break
        </button>
        <button
          onClick={onConfirm}
          className="rounded-lg bg-gray-700 px-8 py-3 font-semibold text-white transition-colors hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 focus:ring-offset-gray-900"
        >
          Yes, Continue
        </button>
      </div>

      {/* Skip hint */}
      <div className="text-center text-xs text-gray-500">
        <p>Pressing any button will reset your break timer</p>
      </div>
    </div>
  );
}
