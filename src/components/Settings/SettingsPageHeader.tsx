import type { RefObject } from "react";
import { BackArrowIcon } from "../icons";

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
        <button
          type="button"
          onClick={onBackButtonClick}
          data-testid={backButtonTestId}
          aria-label={backButtonLabel}
          title={backButtonLabel}
          className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-transparent text-gray-600 transition-colors hover:text-gray-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500"
        >
          <BackArrowIcon className="h-5 w-5" />
        </button>

        <h1
          ref={headingRef}
          tabIndex={-1}
          data-testid={headingTestId}
          className="text-2xl font-bold tracking-tight text-gray-900"
        >
          {title}
        </h1>
      </div>
      {description ? (
        <p className="max-w-xl text-sm leading-6 text-gray-600">
          {description}
        </p>
      ) : null}
    </header>
  );
}
