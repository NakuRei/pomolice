import type { AriaAttributes, AriaRole, ReactElement, ReactNode } from "react";

export type StatusNoticeTone = "info" | "warning" | "danger" | "success";

const STATUS_NOTICE_BASE_CLASS_NAME = "rounded-md border px-4 py-3 text-sm";
const STATUS_NOTICE_CONTENT_ROW_CLASS_NAME = "flex items-start gap-3";

const STATUS_NOTICE_TONE_CLASS_NAME_BY_TONE: Record<StatusNoticeTone, string> =
  {
    info: "border-info-800 bg-info-950/30 text-info-100",
    warning: "border-warning-800 bg-warning-950/30 text-warning-200",
    danger: "border-danger-800 bg-danger-950/30 text-danger-200",
    success: "border-success-800 bg-success-950/30 text-success-200",
  };

/**
 * Shared visual shell for tone-based status notices.
 *
 * Usage example:
 * ```tsx
 * <StatusNotice
 *   tone="danger"
 *   role="alert"
 *   icon={<WarningTriangleIcon className="mt-0.5 h-4 w-4 shrink-0 text-danger-400" />}
 *   actions={<button type="button">Retry</button>}
 *   className="mb-4"
 *   testId="inline-error"
 * >
 *   緊急停止に失敗しました。もう一度試してください。
 * </StatusNotice>
 * ```
 */
interface StatusNoticeProps {
  /** Semantic tone controlling the border, background, and foreground classes. */
  tone: StatusNoticeTone;
  /** Main notice content. Keep the user-readable text in this slot. */
  children: ReactNode;
  /** Optional leading visual. It remains decorative and does not contribute accessible text. */
  icon?: ReactElement | null;
  /** Optional trailing action area such as retry controls. */
  actions?: ReactElement | null;
  /** Optional ARIA role. Callers decide whether the notice is a `status` or `alert`. */
  role?: AriaRole;
  /** Optional live region politeness. Callers opt into announcements explicitly. */
  ariaLive?: AriaAttributes["aria-live"];
  /** Additional wrapper classes such as outer margin or width constraints. */
  className?: string;
  /** Optional stable selector for existing tests. */
  testId?: string;
}

export function StatusNotice({
  tone,
  children,
  icon,
  actions,
  role,
  ariaLive,
  className = "",
  testId,
}: StatusNoticeProps): React.ReactElement {
  const noticeClassName = [
    STATUS_NOTICE_BASE_CLASS_NAME,
    STATUS_NOTICE_TONE_CLASS_NAME_BY_TONE[tone],
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      role={role}
      aria-live={ariaLive}
      className={noticeClassName}
      data-testid={testId}
    >
      <div className={STATUS_NOTICE_CONTENT_ROW_CLASS_NAME}>
        {icon !== undefined && icon !== null ? (
          <div aria-hidden="true">{icon}</div>
        ) : null}
        <div className="min-w-0 flex-1">{children}</div>
        {actions !== undefined && actions !== null ? (
          <div className="shrink-0">{actions}</div>
        ) : null}
      </div>
    </div>
  );
}
