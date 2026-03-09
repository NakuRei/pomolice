/**
 * Shared cancel button for challenge-style Skip Flow steps.
 */
interface SkipFlowCancelButtonProps {
  onCancel: () => void;
}

export default function SkipFlowCancelButton({
  onCancel,
}: SkipFlowCancelButtonProps) {
  return (
    <button
      onClick={onCancel}
      className="rounded-lg bg-gray-700 px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 focus:ring-offset-gray-900"
    >
      Cancel and Take Break
    </button>
  );
}
