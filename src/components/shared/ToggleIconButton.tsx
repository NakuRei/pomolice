import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { IconButton, type IconButtonAppearance } from "./IconButton";

/**
 * Shared primitive for icon-only toggle buttons.
 *
 * Usage examples:
 * ```tsx
 * <ToggleIconButton
 *   label="通知音"
 *   pressed={muted}
 *   onPressedChange={setMuted}
 *   pressedAppearance="danger"
 *   pressedChildren={<VolumeMutedIcon className="h-5 w-5" />}
 * >
 *   <VolumeOnIcon className="h-5 w-5" />
 * </ToggleIconButton>
 * ```
 */
interface ToggleIconButtonProps
  extends Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    "aria-label" | "aria-pressed" | "children" | "className" | "onClick"
  > {
  /** Stable accessible name for the toggle. */
  label: string;
  /** Current pressed state exposed through `aria-pressed`. */
  pressed: boolean;
  /** Called with the next pressed state after a click. */
  onPressedChange: (pressed: boolean) => void;
  /** Shared color token for the unpressed state. */
  appearance?: IconButtonAppearance;
  /** Shared color token for the pressed state. */
  pressedAppearance?: IconButtonAppearance;
  /** Optional class name for layout-only adjustments such as `shrink-0`. */
  className?: string;
  /** Optional test id mapped to `data-testid`. */
  testId?: string;
  /** Icon rendered when `pressed` is false. */
  children: ReactNode;
  /** Optional icon rendered when `pressed` is true. */
  pressedChildren?: ReactNode;
}

export const ToggleIconButton = forwardRef<
  HTMLButtonElement,
  ToggleIconButtonProps
>(function ToggleIconButton(
  {
    label,
    pressed,
    onPressedChange,
    appearance = "neutral",
    pressedAppearance = appearance,
    className = "",
    testId,
    children,
    pressedChildren,
    ...buttonProps
  },
  ref,
) {
  const handleClick = () => {
    onPressedChange(!pressed);
  };

  return (
    <IconButton
      {...buttonProps}
      ref={ref}
      label={label}
      appearance={pressed ? pressedAppearance : appearance}
      className={className}
      testId={testId}
      aria-pressed={pressed}
      onClick={handleClick}
    >
      {pressed ? pressedChildren ?? children : children}
    </IconButton>
  );
});
