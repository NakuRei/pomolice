/**
 * Final Warning Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Step 10: Final warning with SNS notification
 * Last chance to reconsider before actually skipping the break
 */

interface FinalWarningProps {
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Final warning step before completing skip flow
 */
export default function FinalWarning({
  onConfirm,
  onCancel,
}: FinalWarningProps) {
  return (
    <div className="space-y-8">
      {/* Final warning header */}
      <div className="text-center">
        <h2 className="mb-2 text-3xl font-bold text-danger-400">Final Warning</h2>
        <p className="text-lg text-text-muted">
          You are about to skip this break.
        </p>
      </div>

      {/* Warning messages */}
      <div className="space-y-4">
        {/* Skip will be recorded */}
        <div className="rounded-lg border-2 border-warning-500 bg-warning-950/30 p-4">
          <p className="font-semibold text-warning-300">
            ⚠️ This skip will be recorded in your statistics
          </p>
          <p className="mt-2 text-sm text-warning-200">
            Your break completion rate will decrease, and this action will be
            tracked in your daily report.
          </p>
        </div>

        {/* Health reminder */}
        <div className="rounded-lg border-2 border-danger-500 bg-danger-950/30 p-4">
          <p className="font-semibold text-danger-300">
            💔 Skipping breaks harms your health and productivity
          </p>
          <p className="mt-2 text-sm text-danger-200">
            Regular breaks are essential for:
          </p>
          <ul className="mt-2 list-inside list-disc space-y-1 text-sm text-danger-200">
            <li>Reducing eye strain and preventing headaches</li>
            <li>Maintaining focus and mental clarity</li>
            <li>Preventing burnout and fatigue</li>
            <li>Improving long-term productivity</li>
          </ul>
        </div>
      </div>

      {/* Final question */}
      <div className="rounded-lg bg-surface-raised p-6 text-center">
        <p className="text-xl font-bold text-text">
          Are you absolutely certain you want to skip this break?
        </p>
      </div>

      {/* Action buttons */}
      <div className="flex justify-center space-x-4">
        <button
          onClick={onCancel}
          className="flex-1 rounded-lg bg-success-700 px-8 py-4 text-lg font-bold text-on-success transition-colors hover:bg-success-800 focus:outline-none focus:ring-4 focus:ring-success-600 focus:ring-offset-2 focus:ring-offset-surface"
        >
          No, Take the Break
        </button>
        <button
          onClick={onConfirm}
          className="flex-1 rounded-lg border-2 border-danger-500 bg-danger-950/50 px-8 py-4 text-lg font-bold text-danger-300 transition-colors hover:bg-danger-950/70 focus:outline-none focus:ring-4 focus:ring-danger-500 focus:ring-offset-2 focus:ring-offset-surface"
        >
          Yes, Skip Anyway
        </button>
      </div>

      {/* Encouragement message */}
      <div className="text-center text-sm text-text-muted">
        <p className="italic">
          "It only takes 5 minutes to rest. Your future self will thank you."
        </p>
      </div>
    </div>
  );
}
