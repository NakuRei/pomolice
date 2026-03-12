import type { ReactNode } from "react";

interface DialogOverlayProps {
  children: ReactNode;
  className?: string;
  testId?: string;
}

const DIALOG_OVERLAY_BASE_CLASS_NAME =
  "fixed inset-0 z-50 flex items-center justify-center bg-black/50";

/**
 * Shared overlay shell for confirmation dialogs.
 *
 * Usage example:
 * ```tsx
 * <DialogOverlay testId="confirm-overlay">
 *   <DialogPanel labelledBy="confirm-title">...</DialogPanel>
 * </DialogOverlay>
 * ```
 */
export function DialogOverlay({
  children,
  className = "",
  testId,
}: DialogOverlayProps) {
  const overlayClassName = [DIALOG_OVERLAY_BASE_CLASS_NAME, className]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={overlayClassName} data-testid={testId}>
      {children}
    </div>
  );
}
