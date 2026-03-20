import type { ReactNode } from "react";

interface DialogActionRowProps {
  children: ReactNode;
  className?: string;
  testId?: string;
}

const DIALOG_ACTION_ROW_BASE_CLASS_NAME = "flex gap-4";

/**
 * Shared horizontal action row for confirmation dialog buttons.
 *
 * Usage example:
 * ```tsx
 * <DialogActionRow>
 *   <button type="button">Cancel</button>
 *   <button type="button">Confirm</button>
 * </DialogActionRow>
 * ```
 */
export function DialogActionRow({
  children,
  className = "",
  testId,
}: DialogActionRowProps): React.ReactElement {
  const actionRowClassName = [DIALOG_ACTION_ROW_BASE_CLASS_NAME, className]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={actionRowClassName} data-testid={testId}>
      {children}
    </div>
  );
}
