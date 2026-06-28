import type { ReactNode } from "react";

interface DialogPanelProps {
  children: ReactNode;
  labelledBy?: string;
  describedBy?: string;
  className?: string;
  testId?: string;
}

const DIALOG_PANEL_BASE_CLASS_NAME =
  "mx-4 w-full max-w-md rounded-lg border border-border bg-surface p-8 text-on-surface shadow-xl shadow-shadow/30";

function getClassTokens(className: string): string[] {
  return className.split(/\s+/u).filter(Boolean);
}

function isShadowSizeClassName(token: string): boolean {
  return /^(?:shadow|shadow-(?:none|xs|sm|md|lg|xl|2xl))$/u.test(token);
}

function isShadowColorClassName(token: string): boolean {
  return token.startsWith("shadow-shadow/");
}

function buildPanelClassName(className: string): string {
  const overrideTokens = getClassTokens(className);
  const hasWidthOverride = overrideTokens.some((token) =>
    token.startsWith("max-w-"),
  );
  const hasShadowSizeOverride = overrideTokens.some(isShadowSizeClassName);
  const hasShadowColorOverride = overrideTokens.some(isShadowColorClassName);
  const baseTokens = getClassTokens(DIALOG_PANEL_BASE_CLASS_NAME).filter(
    (token) => {
      if (hasWidthOverride && token.startsWith("max-w-")) {
        return false;
      }

      if (hasShadowSizeOverride && isShadowSizeClassName(token)) {
        return false;
      }

      if (hasShadowColorOverride && isShadowColorClassName(token)) {
        return false;
      }

      return true;
    },
  );

  return [...baseTokens, ...overrideTokens].join(" ");
}

/**
 * Accessible confirmation dialog panel with the shared surface-card appearance.
 */
export function DialogPanel({
  children,
  labelledBy,
  describedBy,
  className = "",
  testId,
}: DialogPanelProps): React.ReactElement {
  const panelClassName = buildPanelClassName(className);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      tabIndex={-1}
      className={panelClassName}
      data-testid={testId}
    >
      {children}
    </div>
  );
}
