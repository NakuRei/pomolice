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
      className="rounded-lg bg-surface-raised px-6 py-2 text-sm font-medium text-on-surface transition-colors hover:bg-surface-hover focus:outline-none focus:ring-2 focus:ring-primary-600 focus:ring-offset-2 focus:ring-offset-surface"
    >
      Cancel and Take Break
    </button>
  );
}
