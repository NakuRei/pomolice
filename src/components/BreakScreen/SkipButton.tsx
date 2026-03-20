/**
 * SkipButton Component
 *
 * A small, hard-to-press button for skipping breaks.
 * Intentionally designed to be inconvenient to discourage skipping.
 *
 * Design requirements (from SPECIFICATION.md):
 * - Small size (hard to click)
 * - Positioned in bottom-right corner (not prominent)
 * - Triggers multi-step confirmation flow (Phase 5)
 */

interface SkipButtonProps {
  onClick: () => void;
}

export function SkipButton({ onClick }: SkipButtonProps): React.ReactElement {
  return (
    <button
      onClick={onClick}
      className="rounded bg-surface-raised px-3 py-1.5 text-xs text-text-muted opacity-60 transition-colors hover:bg-surface-hover hover:text-text hover:opacity-100"
      aria-label="休憩をスキップ"
    >
      スキップ
    </button>
  );
}
