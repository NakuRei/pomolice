import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

export type ButtonAppearance =
  "primary" | "secondary" | "outline" | "danger" | "subtle";
export type ButtonSize = "sm" | "md";

const BUTTON_BASE_CLASS_NAME =
  "inline-flex items-center justify-center gap-2.5 rounded-full border font-medium tracking-wider transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 disabled:cursor-not-allowed";

const BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE: Record<ButtonAppearance, string> =
  {
    primary:
      "border-transparent bg-accent text-on-accent hover:bg-accent-hover focus-visible:outline-accent-ring disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    secondary:
      "border-border bg-surface-raised text-on-surface hover:border-border-strong hover:bg-surface-hover focus-visible:outline-accent-ring disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    danger:
      "border-transparent bg-danger-600 text-on-danger hover:bg-danger-700 focus-visible:outline-danger-500 disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    subtle:
      "border-transparent bg-transparent text-text-muted hover:bg-surface hover:text-text focus-visible:outline-accent-ring disabled:text-disabled-text disabled:hover:bg-transparent",
    outline:
      "border-accent/35 bg-transparent text-accent-soft hover:bg-accent/10 focus-visible:outline-accent-ring disabled:border-disabled-border disabled:text-disabled-text disabled:hover:bg-transparent",
  };

const BUTTON_SIZE_CLASS_NAME_BY_VALUE: Record<ButtonSize, string> = {
  sm: "min-h-8 px-4 py-1 text-sm",
  md: "min-h-11 px-6 py-2 text-base",
};

function buildButtonClassName(
  appearance: ButtonAppearance,
  size: ButtonSize,
  className: string,
): string {
  return [
    BUTTON_BASE_CLASS_NAME,
    BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE[appearance],
    BUTTON_SIZE_CLASS_NAME_BY_VALUE[size],
    className,
  ]
    .filter(Boolean)
    .join(" ");
}

/**
 * Shared primitive for standard text buttons.
 *
 * Usage examples:
 * ```tsx
 * <Button appearance="primary">Save</Button>
 * <Button appearance="secondary" size="sm">Retry</Button>
 * <Button appearance="danger" className="flex-1">Delete</Button>
 * ```
 */
interface ButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "className"
> {
  /** Visual emphasis level for standard text buttons. */
  appearance?: ButtonAppearance;
  /**
   * Size follows placement density, never importance (use `appearance` for that):
   * `md` keeps the 44px pointer target from Apple HIG / WCAG 2.5.5, `sm` is only
   * for buttons inline with text or form controls where 44px would dominate the row.
   */
  size?: ButtonSize;
  /** Optional class name for layout-only adjustments such as `flex-1`. */
  className?: string;
  /** Optional test id mapped to `data-testid`. */
  testId?: string;
  children: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      appearance = "secondary",
      size = "md",
      className = "",
      testId,
      type = "button",
      children,
      ...buttonProps
    },
    ref,
  ) {
    const buttonClassName = buildButtonClassName(appearance, size, className);

    return (
      <button
        {...buttonProps}
        ref={ref}
        type={type}
        className={buttonClassName}
        data-testid={testId}
      >
        {children}
      </button>
    );
  },
);
