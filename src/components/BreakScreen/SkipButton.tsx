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

export function SkipButton({ onClick }: SkipButtonProps) {
  return (
    <button
      onClick={onClick}
      className="
        px-3 py-1.5
        text-xs
        bg-surface-raised hover:bg-surface-hover
        text-text-muted hover:text-text
        rounded
        transition-colors
        opacity-60 hover:opacity-100
      "
      aria-label="休憩をスキップ"
    >
      スキップ
    </button>
  );
}
