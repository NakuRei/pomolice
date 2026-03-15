import type { ReactNode } from "react";
import { IconButton } from "./IconButton";

interface ActionIconButtonProps {
  label: string;
  testId: string;
  onClick: () => void;
  children: ReactNode;
  className?: string;
}

export function ActionIconButton({
  label,
  testId,
  onClick,
  children,
  className = "",
}: ActionIconButtonProps) {
  return (
    <IconButton
      onClick={onClick}
      className={className}
      label={label}
      testId={testId}
    >
      {children}
    </IconButton>
  );
}
