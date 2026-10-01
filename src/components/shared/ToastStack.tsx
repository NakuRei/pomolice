import type { ReactNode } from "react";

/**
 * Placed clear of the header icons and the footer action so a notice never
 * covers a control. Notices that stay until resolved belong in the page flow.
 *
 * Usage example:
 * ```tsx
 * <ToastStack>
 *   <InfoMessageBanner />
 *   <OperationNotice operation="start_work" />
 * </ToastStack>
 * ```
 */
interface ToastStackProps {
  children: ReactNode;
}

export function ToastStack({ children }: ToastStackProps): React.ReactElement {
  return (
    <div
      className="pointer-events-none fixed inset-x-4 bottom-4 z-50 flex max-w-sm flex-col gap-2 *:pointer-events-auto"
      data-testid="toast-stack"
    >
      {children}
    </div>
  );
}
