import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface DialogOverlayProps {
  children: ReactNode;
  className?: string;
  testId?: string;
}

const DIALOG_OVERLAY_BASE_CLASS_NAME =
  "fixed inset-0 z-50 flex items-center justify-center bg-backdrop/50";

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

function getDialogRoot(overlayElement: HTMLElement | null): HTMLElement | null {
  if (overlayElement === null) {
    return null;
  }

  const dialogElement =
    overlayElement.querySelector<HTMLElement>("[role='dialog']");
  return dialogElement instanceof HTMLElement ? dialogElement : null;
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

/**
 * Shared overlay shell for confirmation dialogs.
 *
 * Usage example:
 * ```tsx
 * <DialogOverlay testId="confirm-overlay">
 *   <DialogPanel labelledBy="confirm-title">...</DialogPanel>
 * </DialogOverlay>
 * ```
 */
export function DialogOverlay({
  children,
  className = "",
  testId,
}: DialogOverlayProps): React.ReactElement {
  const overlayReference = useRef<HTMLDivElement>(null);
  const modalIdReference = useRef(Symbol("dialog-overlay"));
  const previousFocusTargetReference = useRef<HTMLElement | null>(
    typeof document === "undefined" ? null : getActiveElement(),
  );
  const overlayClassName = buildOverlayClassName(className);

  useEffect(() => {
    const modalId = modalIdReference.current;
    const overlayElement = overlayReference.current;

    if (overlayElement === null) {
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

      const dialogElement = getDialogRoot(overlayElement);
      if (dialogElement === null) {
        return;
      }

      trapFocusWithinDialog(event, dialogElement);
    };

    document.addEventListener("keydown", handleKeyDown);

    const dialogElement = getDialogRoot(overlayElement);
    if (dialogElement !== null) {
      const activeElement = getActiveElement();
      if (activeElement === null || !dialogElement.contains(activeElement)) {
        dialogElement.focus();
      }
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
    <div
      ref={overlayReference}
      className={overlayClassName}
      data-testid={testId}
    >
      {children}
    </div>,
    document.body,
  );
}
