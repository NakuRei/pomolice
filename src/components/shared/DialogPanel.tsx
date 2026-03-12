import type { ReactNode } from "react";

interface DialogPanelProps {
  children: ReactNode;
  labelledBy?: string;
  describedBy?: string;
  className?: string;
  testId?: string;
}

const DIALOG_PANEL_BASE_CLASS_NAME =
  "mx-4 w-full max-w-md rounded-lg bg-white p-8 shadow-xl";

/**
 * Accessible confirmation dialog panel with the shared white-card appearance.
 *
 * Usage example:
 * ```tsx
 * <DialogPanel labelledBy="confirm-title" testId="confirm-dialog">
 *   <h2 id="confirm-title">Confirm action</h2>
 * </DialogPanel>
 * ```
 */
export function DialogPanel({
  children,
  labelledBy,
  describedBy,
  className = "",
  testId,
}: DialogPanelProps) {
  const panelClassName = [DIALOG_PANEL_BASE_CLASS_NAME, className]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      className={panelClassName}
      data-testid={testId}
    >
      {children}
    </div>
  );
}
