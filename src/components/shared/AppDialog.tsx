import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { TimerEffectsNotice } from "@/app/ui/TimerEffectsNotice";

interface AppDialogProps {
  children: ReactNode;
  labelledBy: string;
  describedBy?: string;
  className?: string;
  overlayClassName?: string;
  testId?: string;
  overlayTestId?: string;
}

const DIALOG_OVERLAY_BASE_CLASS_NAME =
  "fixed inset-0 z-50 flex items-center justify-center bg-backdrop/50";

const DIALOG_PANEL_BASE_CLASS_NAME =
  "mx-4 max-h-[calc(100dvh-2rem)] w-full max-w-md overflow-y-auto rounded-lg border border-border bg-surface p-8 text-on-surface shadow-xl shadow-shadow/30";

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

interface ModalStackEntry {
  id: symbol;
  previousFocusTarget: HTMLElement | null;
}

const modalStack: ModalStackEntry[] = [];

type RootElementWithInert = HTMLElement & {
  inert?: boolean;
};

function getClassTokens(className: string): string[] {
  return className.split(/\s+/u).filter(Boolean);
}

function buildOverlayClassName(className: string): string {
  const overrideTokens = getClassTokens(className);
  const hasBackdropOverride = overrideTokens.some((token) =>
    token.startsWith("bg-backdrop/"),
  );
  const baseTokens = getClassTokens(DIALOG_OVERLAY_BASE_CLASS_NAME).filter(
    (token) => !hasBackdropOverride || !token.startsWith("bg-backdrop/"),
  );

  return [...baseTokens, ...overrideTokens].join(" ");
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

function getActiveElement(): HTMLElement | null {
  return document.activeElement instanceof HTMLElement
    ? document.activeElement
    : null;
}

function getRootElement(): RootElementWithInert | null {
  const rootElement = document.getElementById("root");
  return rootElement instanceof HTMLElement ? rootElement : null;
}

function setRootVisibilityState(
  rootElement: RootElementWithInert,
  isHidden: boolean,
): void {
  rootElement.inert = isHidden;

  if (isHidden) {
    rootElement.setAttribute("aria-hidden", "true");
    rootElement.setAttribute("inert", "");
    return;
  }

  rootElement.removeAttribute("aria-hidden");
  rootElement.removeAttribute("inert");
}

function getFocusableElements(dialogElement: HTMLElement): HTMLElement[] {
  return Array.from(
    dialogElement.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  ).filter(
    (element) => element.tabIndex >= 0 && !element.hasAttribute("disabled"),
  );
}

function canRestoreFocus(target: HTMLElement | null): target is HTMLElement {
  if (target === null) {
    return false;
  }

  if (!document.contains(target) || typeof target.focus !== "function") {
    return false;
  }

  return !(
    "disabled" in target && (target as { disabled?: boolean }).disabled === true
  );
}

function isActiveElementOutsideDialog(
  dialogElement: HTMLElement,
  activeElement: HTMLElement | null,
): boolean {
  if (activeElement === null) {
    return true;
  }

  if (activeElement === dialogElement) {
    return true;
  }

  return !dialogElement.contains(activeElement);
}

function resolveTrapFocusTarget(
  dialogElement: HTMLElement,
  focusableElements: HTMLElement[],
  isShiftKeyPressed: boolean,
): HTMLElement | null {
  const firstFocusableElement = focusableElements[0];
  const lastFocusableElement = focusableElements[focusableElements.length - 1];

  if (
    firstFocusableElement === undefined ||
    lastFocusableElement === undefined
  ) {
    return dialogElement;
  }

  const activeElement = getActiveElement();

  if (isActiveElementOutsideDialog(dialogElement, activeElement)) {
    return isShiftKeyPressed ? lastFocusableElement : firstFocusableElement;
  }

  if (isShiftKeyPressed && activeElement === firstFocusableElement) {
    return lastFocusableElement;
  }

  if (!isShiftKeyPressed && activeElement === lastFocusableElement) {
    return firstFocusableElement;
  }

  return null;
}

function trapFocusWithinDialog(
  event: KeyboardEvent,
  dialogElement: HTMLElement,
): void {
  const focusableElements = getFocusableElements(dialogElement);

  if (focusableElements.length === 0) {
    event.preventDefault();
    dialogElement.focus();
    return;
  }

  const targetElement = resolveTrapFocusTarget(
    dialogElement,
    focusableElements,
    event.shiftKey,
  );

  if (targetElement === null) {
    return;
  }

  event.preventDefault();
  targetElement.focus();
}

export function AppDialog({
  children,
  labelledBy,
  describedBy,
  className = "",
  overlayClassName = "",
  testId,
  overlayTestId,
}: AppDialogProps): React.ReactElement {
  const dialogReference = useRef<HTMLDivElement>(null);
  const modalIdReference = useRef(Symbol("app-dialog"));
  const previousFocusTargetReference = useRef<HTMLElement | null>(
    typeof document === "undefined" ? null : getActiveElement(),
  );
  const overlayClasses = buildOverlayClassName(overlayClassName);
  const panelClassName = buildPanelClassName(className);

  useEffect(() => {
    const modalId = modalIdReference.current;
    const dialogElement = dialogReference.current;

    if (dialogElement === null) {
      return;
    }

    const stackEntry: ModalStackEntry = {
      id: modalId,
      previousFocusTarget: previousFocusTargetReference.current,
    };

    modalStack.push(stackEntry);

    const rootElement = getRootElement();
    if (rootElement !== null) {
      setRootVisibilityState(rootElement, true);
    }

    const handleKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== "Tab") {
        return;
      }

      const topModal = modalStack[modalStack.length - 1];
      if (topModal?.id !== modalId) {
        return;
      }

      trapFocusWithinDialog(event, dialogElement);
    };

    document.addEventListener("keydown", handleKeyDown);

    const activeElement = getActiveElement();
    if (activeElement === null || !dialogElement.contains(activeElement)) {
      dialogElement.focus();
    }

    return (): void => {
      document.removeEventListener("keydown", handleKeyDown);

      const stackIndex = modalStack.findIndex((entry) => entry.id === modalId);
      const isTopModal = stackIndex === modalStack.length - 1;
      const [closedEntry] =
        stackIndex >= 0 ? modalStack.splice(stackIndex, 1) : [];

      const nextRootElement = getRootElement();
      if (modalStack.length === 0 && nextRootElement !== null) {
        setRootVisibilityState(nextRootElement, false);
      }

      if (
        isTopModal &&
        closedEntry !== undefined &&
        canRestoreFocus(closedEntry.previousFocusTarget)
      ) {
        queueMicrotask(() => {
          if (canRestoreFocus(closedEntry.previousFocusTarget)) {
            closedEntry.previousFocusTarget.focus();
          }
        });
      }
    };
  }, []);

  return createPortal(
    <div className={overlayClasses} data-testid={overlayTestId}>
      <div
        ref={dialogReference}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        tabIndex={-1}
        className={panelClassName}
        data-testid={testId}
      >
        <div className="mb-4 empty:hidden">
          <TimerEffectsNotice />
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}
