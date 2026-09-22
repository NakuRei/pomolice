import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

export type ButtonAppearance =
  | "primary"
  | "secondary"
  | "outline"
  | "danger"
  | "subtle";
export type ButtonSize = "sm" | "md" | "lg";
export type ButtonWidth = "auto" | "full";
export type ButtonShape = "rounded" | "pill";

const BUTTON_BASE_CLASS_NAME =
  "inline-flex items-center justify-center border font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 disabled:cursor-not-allowed";

const BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE: Record<ButtonAppearance, string> =
  {
    primary:
      "border-transparent bg-accent text-on-accent hover:bg-accent-hover focus-visible:outline-accent-ring disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    secondary:
      "border-border bg-surface-raised text-on-surface hover:border-border-strong hover:bg-surface-hover focus-visible:outline-accent-ring disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    danger:
      "border-transparent bg-danger-600 text-on-danger hover:bg-danger-700 focus-visible:outline-danger-500 disabled:bg-disabled-bg disabled:text-disabled-text disabled:focus-visible:outline-disabled-border",
    subtle:
      "border-transparent bg-transparent text-text-muted hover:bg-surface hover:text-text focus-visible:outline-accent-ring disabled:text-disabled-text disabled:hover:bg-transparent",
    outline:
      "border-accent/35 bg-transparent text-accent-soft hover:bg-accent/10 focus-visible:outline-accent-ring disabled:border-disabled-border disabled:text-disabled-text disabled:hover:bg-transparent",
  };

const BUTTON_SIZE_CLASS_NAME_BY_VALUE: Record<ButtonSize, string> = {
  sm: "px-4 py-2 text-sm",
  md: "px-6 py-3 text-base",
  lg: "px-8 py-4 text-lg",
};

const BUTTON_SHAPE_CLASS_NAME_BY_VALUE: Record<ButtonShape, string> = {
  rounded: "rounded-lg",
  pill: "gap-2.5 rounded-full tracking-wider",
};

const BUTTON_WIDTH_CLASS_NAME_BY_VALUE: Record<ButtonWidth, string> = {
  auto: "",
  full: "w-full",
};

function buildButtonClassName(
  appearance: ButtonAppearance,
  size: ButtonSize,
  width: ButtonWidth,
  shape: ButtonShape,
  className: string,
): string {
  return [
    BUTTON_BASE_CLASS_NAME,
    BUTTON_APPEARANCE_CLASS_NAME_BY_VALUE[appearance],
    BUTTON_SIZE_CLASS_NAME_BY_VALUE[size],
    BUTTON_WIDTH_CLASS_NAME_BY_VALUE[width],
    BUTTON_SHAPE_CLASS_NAME_BY_VALUE[shape],
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
 * <Button appearance="secondary" size="sm">Cancel</Button>
 * <Button appearance="outline" shape="pill">Skip</Button>
 * <Button appearance="danger" width="full">Delete</Button>
 * ```
 */
interface ButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "className"
> {
  /** Visual emphasis level for standard text buttons. */
  appearance?: ButtonAppearance;
  /** Shared size token for spacing, border radius, and text size. */
  size?: ButtonSize;
  /** Shared width token for standard text buttons. */
  width?: ButtonWidth;
  /** Corner treatment; `pill` is the fully rounded screen action with icon spacing. */
  shape?: ButtonShape;
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
      width = "auto",
      shape = "rounded",
      className = "",
      testId,
      type = "button",
      children,
      ...buttonProps
    },
    ref,
  ) {
    const buttonClassName = buildButtonClassName(
      appearance,
      size,
      width,
      shape,
      className,
    );

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
