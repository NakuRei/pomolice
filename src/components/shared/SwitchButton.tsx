import type { ButtonHTMLAttributes } from "react";

const SWITCH_TRACK_BASE_CLASS_NAME =
  "inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-ring";

const SWITCH_THUMB_BASE_CLASS_NAME =
  "pointer-events-none inline-block h-4 w-4 rounded-full shadow-sm transition-transform";

interface SwitchButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "aria-checked" | "children" | "className" | "onClick" | "role" | "type"
> {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}

export function SwitchButton({
  checked,
  onCheckedChange,
  ...buttonProps
}: SwitchButtonProps): React.ReactElement {
  const trackClassName = `${SWITCH_TRACK_BASE_CLASS_NAME} ${
    checked
      ? "border-transparent bg-accent hover:bg-accent-hover"
      : "border-border-strong bg-surface-raised hover:border-accent-ring"
  }`;
  const thumbClassName = `${SWITCH_THUMB_BASE_CLASS_NAME} ${
    checked ? "translate-x-6 bg-on-accent" : "translate-x-1 bg-text-muted"
  }`;

  return (
    <button
      {...buttonProps}
      type="button"
      role="switch"
      aria-checked={checked}
      className={trackClassName}
      onClick={() => {
        onCheckedChange(!checked);
      }}
    >
      <span aria-hidden="true" className={thumbClassName} />
    </button>
  );
}
