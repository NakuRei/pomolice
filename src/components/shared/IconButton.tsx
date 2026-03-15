import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

export type IconButtonAppearance = "neutral" | "danger";

const ICON_BUTTON_BASE_CLASS_NAME =
  "inline-flex h-11 w-11 items-center justify-center rounded-full bg-transparent transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 disabled:cursor-not-allowed";

const ICON_BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE: Record<
  IconButtonAppearance,
  string
> = {
  neutral:
    "text-text-muted hover:text-text focus-visible:outline-primary-600 disabled:text-disabled-text",
  danger:
    "text-danger-500 hover:text-danger-400 focus-visible:outline-danger-500 disabled:text-disabled-text",
};

/**
 * Shared primitive for icon-only action buttons.
 *
 * Usage examples:
 * ```tsx
 * <IconButton label="設定" onClick={openSettings}>
 *   <SettingsGearIcon className="h-6 w-6" />
 * </IconButton>
 *
 * <IconButton label="削除" appearance="danger" disabled>
 *   <TrashIcon className="h-5 w-5" />
 * </IconButton>
 * ```
 */
interface IconButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "className"
> {
  /** Stable accessible name for screen readers and tooltips. */
  label: string;
  /** Shared color token for icon-only buttons. */
  appearance?: IconButtonAppearance;
  /** Optional class name for layout-only adjustments such as `shrink-0`. */
  className?: string;
  /** Optional test id mapped to `data-testid`. */
  testId?: string;
  children: ReactNode;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton(
    {
      label,
      appearance = "neutral",
      className = "",
      testId,
      type = "button",
      title = label,
      children,
      ...buttonProps
    },
    ref,
  ) {
    const buttonClassName = [
      ICON_BUTTON_BASE_CLASS_NAME,
      ICON_BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE[appearance],
      className,
    ]
      .filter(Boolean)
      .join(" ");

    return (
      <button
        {...buttonProps}
        ref={ref}
        type={type}
        className={buttonClassName}
        data-testid={testId}
        aria-label={label}
        title={title}
      >
        <span aria-hidden="true" className="flex items-center justify-center">
          {children}
        </span>
      </button>
    );
  },
);
