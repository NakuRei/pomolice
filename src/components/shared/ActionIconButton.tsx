import type { ReactNode } from "react";

interface ActionIconButtonProps {
  label: string;
  testId: string;
  onClick: () => void;
  children: ReactNode;
  className?: string;
}

const ACTION_ICON_BUTTON_BASE_CLASS_NAME =
  "flex h-11 w-11 items-center justify-center rounded-full bg-transparent text-gray-600 transition-colors hover:text-gray-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500";

export function ActionIconButton({
  label,
  testId,
  onClick,
  children,
  className = "",
}: ActionIconButtonProps) {
  const buttonClassName =
    `${ACTION_ICON_BUTTON_BASE_CLASS_NAME} ${className}`.trim();

  return (
    <button
      type="button"
      onClick={onClick}
      className={buttonClassName}
      title={label}
      data-testid={testId}
      aria-label={label}
    >
      {children}
    </button>
  );
}
