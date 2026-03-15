import type { RefObject } from "react";
import { BackArrowIcon } from "../icons";
import { IconButton } from "../shared/IconButton";

interface SettingsPageHeaderProps {
  title: string;
  headingTestId: string;
  headingRef: RefObject<HTMLHeadingElement | null>;
  backButtonLabel: string;
  backButtonTestId: string;
  onBackButtonClick: () => void;
  description?: string;
}

export function SettingsPageHeader({
  title,
  headingTestId,
  headingRef,
  backButtonLabel,
  backButtonTestId,
  onBackButtonClick,
  description,
}: SettingsPageHeaderProps) {
  return (
    <header className="mb-6 space-y-2">
      <div className="flex items-center gap-3">
        <IconButton
          label={backButtonLabel}
          testId={backButtonTestId}
          onClick={onBackButtonClick}
          className="shrink-0"
        >
          <BackArrowIcon className="h-5 w-5" />
        </IconButton>

        <h1
          ref={headingRef}
          tabIndex={-1}
          data-testid={headingTestId}
          className="text-2xl font-bold tracking-tight text-text"
        >
          {title}
        </h1>
      </div>
      {description ? (
        <p className="max-w-xl text-sm leading-6 text-text-muted">
          {description}
        </p>
      ) : null}
    </header>
  );
}
