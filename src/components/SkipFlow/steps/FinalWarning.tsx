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
        <h2 className="mb-2 text-3xl font-bold text-red-400">Final Warning</h2>
        <p className="text-lg text-gray-300">
          You are about to skip this break.
        </p>
      </div>

      {/* Warning messages */}
      <div className="space-y-4">
        {/* Skip will be recorded */}
        <div className="rounded-lg border-2 border-yellow-500 bg-yellow-900 bg-opacity-30 p-4">
          <p className="font-semibold text-yellow-300">
            ⚠️ This skip will be recorded in your statistics
          </p>
          <p className="mt-2 text-sm text-yellow-200">
            Your break completion rate will decrease, and this action will be
            tracked in your daily report.
          </p>
        </div>

        {/* Health reminder */}
        <div className="rounded-lg border-2 border-red-500 bg-red-900 bg-opacity-30 p-4">
          <p className="font-semibold text-red-300">
            💔 Skipping breaks harms your health and productivity
          </p>
          <p className="mt-2 text-sm text-red-200">
            Regular breaks are essential for:
          </p>
          <ul className="mt-2 list-inside list-disc space-y-1 text-sm text-red-200">
            <li>Reducing eye strain and preventing headaches</li>
            <li>Maintaining focus and mental clarity</li>
            <li>Preventing burnout and fatigue</li>
            <li>Improving long-term productivity</li>
          </ul>
        </div>
      </div>

      {/* Final question */}
      <div className="rounded-lg bg-gray-800 p-6 text-center">
        <p className="text-xl font-bold text-white">
          Are you absolutely certain you want to skip this break?
        </p>
      </div>

      {/* Action buttons */}
      <div className="flex justify-center space-x-4">
        <button
          onClick={onCancel}
          className="flex-1 rounded-lg bg-green-600 px-8 py-4 text-lg font-bold text-white transition-colors hover:bg-green-700 focus:outline-none focus:ring-4 focus:ring-green-500 focus:ring-offset-2 focus:ring-offset-gray-900"
        >
          No, Take the Break
        </button>
        <button
          onClick={onConfirm}
          className="flex-1 rounded-lg border-2 border-red-500 bg-red-900 bg-opacity-50 px-8 py-4 text-lg font-bold text-red-300 transition-colors hover:bg-red-900 hover:bg-opacity-70 focus:outline-none focus:ring-4 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-gray-900"
        >
          Yes, Skip Anyway
        </button>
      </div>

      {/* Encouragement message */}
      <div className="text-center text-sm text-gray-500">
        <p className="italic">
          "It only takes 5 minutes to rest. Your future self will thank you."
        </p>
      </div>
    </div>
  );
}
