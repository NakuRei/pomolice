import { StrictMode, useEffect, useRef, useState } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DialogOverlay } from "./DialogOverlay";
import { DialogPanel } from "./DialogPanel";

function createRootFixture(): HTMLDivElement {
  document.body.innerHTML = "";

  const root = document.createElement("div");
  root.id = "root";
  document.body.appendChild(root);

  return root;
}

function renderIntoRoot(
  ui: React.ReactElement,
): ReturnType<typeof render> & { root: HTMLDivElement } {
  const root = createRootFixture();
  const renderResult = render(ui, { container: root });

  return Object.assign(renderResult, { root });
}

function isRootInert(root: HTMLElement): boolean {
  const inertValue = (root as HTMLElement & { inert?: boolean }).inert;

  return inertValue || root.hasAttribute("inert");
}

function expectRootHidden(root: HTMLElement, isHidden: boolean): void {
  if (isHidden) {
    expect(root).toHaveAttribute("aria-hidden", "true");
  } else {
    expect(root).not.toHaveAttribute("aria-hidden");
  }

  expect(isRootInert(root)).toBe(isHidden);
}

function AutoFocusField(): React.ReactElement {
  const inputReference = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputReference.current?.focus();
  }, []);

  return <input ref={inputReference} aria-label="Autofocus field" />;
}

function AutoFocusFirstButton({
  firstLabel,
  secondLabel,
}: {
  firstLabel: string;
  secondLabel: string;
}): React.ReactElement {
  const firstButtonReference = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    firstButtonReference.current?.focus();
  }, []);

  return (
    <>
      <button ref={firstButtonReference}>{firstLabel}</button>
      <button>{secondLabel}</button>
    </>
  );
}

function AutoFocusCloseButton({
  onClose,
}: {
  onClose: () => void;
}): React.ReactElement {
  const closeButtonReference = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeButtonReference.current?.focus();
  }, []);

  return (
    <button ref={closeButtonReference} onClick={onClose}>
      Close nested
    </button>
  );
}

function doNothing(): void {
  return undefined;
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("DialogOverlay accessibility", () => {
  it("marks #root inert and aria-hidden while a dialog overlay is mounted", async () => {
    const user = userEvent.setup();

    function Harness(): React.ReactElement {
      const [isOpen, setIsOpen] = useState(false);

      return (
        <>
          <button
            onClick={() => {
              setIsOpen(true);
            }}
          >
            Open dialog
          </button>
          {isOpen ? (
            <DialogOverlay testId="test-dialog-overlay">
              <DialogPanel labelledBy="test-dialog-title">
                <h2 id="test-dialog-title">Test dialog</h2>
                <button
                  onClick={() => {
                    setIsOpen(false);
                  }}
                >
                  Close dialog
                </button>
              </DialogPanel>
            </DialogOverlay>
          ) : null}
        </>
      );
    }

    const { root } = renderIntoRoot(<Harness />);

    await user.click(screen.getByRole("button", { name: "Open dialog" }));

    expect(
      screen.getByRole("dialog", { name: "Test dialog" }),
    ).toBeInTheDocument();
    expectRootHidden(root, true);
  });

  it("removes #root inert before restoring focus on unmount so focus actually lands on the previous target", async () => {
    const user = userEvent.setup();

    function Harness(): React.ReactElement {
      const [isOpen, setIsOpen] = useState(false);

      return (
        <>
          <button>Previous target</button>
          <button
            onMouseDown={(event) => {
              event.preventDefault();
            }}
            onClick={() => {
              setIsOpen(true);
            }}
          >
            Open dialog
          </button>
          {isOpen ? (
            <DialogOverlay>
              <DialogPanel labelledBy="focus-order-dialog-title">
                <h2 id="focus-order-dialog-title">Focus order dialog</h2>
                <button
                  onClick={() => {
                    setIsOpen(false);
                  }}
                >
                  Close dialog
                </button>
              </DialogPanel>
            </DialogOverlay>
          ) : null}
        </>
      );
    }

    const { root } = renderIntoRoot(<Harness />);
    const previousTarget = screen.getByRole("button", {
      name: "Previous target",
    });
    const nativeFocus = previousTarget.focus.bind(previousTarget);
    const inertStateAtFocusCall: boolean[] = [];

    Object.defineProperty(previousTarget, "focus", {
      configurable: true,
      value: vi.fn(() => {
        inertStateAtFocusCall.push(isRootInert(root));
        nativeFocus();
      }),
    });

    previousTarget.focus();
    expect(previousTarget).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Open dialog" }));
    expectRootHidden(root, true);

    await user.click(screen.getByRole("button", { name: "Close dialog" }));

    await waitFor(() => {
      expect(previousTarget).toHaveFocus();
    });

    expect(inertStateAtFocusCall).not.toContain(true);
    expectRootHidden(root, false);
  });

  it("moves focus to the dialog fallback when opened without tabbable descendants", async () => {
    renderIntoRoot(
      <DialogOverlay>
        <DialogPanel labelledBy="fallback-dialog-title">
          <h2 id="fallback-dialog-title">Fallback dialog</h2>
          <p>No interactive elements here.</p>
        </DialogPanel>
      </DialogOverlay>,
    );

    const dialog = screen.getByRole("dialog", { name: "Fallback dialog" });

    await waitFor(() => {
      expect(dialog).toHaveFocus();
    });
  });

  it("does not override focus when a descendant has already taken focus on mount", async () => {
    renderIntoRoot(
      <DialogOverlay>
        <DialogPanel labelledBy="autofocus-dialog-title">
          <h2 id="autofocus-dialog-title">Autofocus dialog</h2>
          <AutoFocusField />
        </DialogPanel>
      </DialogOverlay>,
    );

    await waitFor(() => {
      expect(
        screen.getByRole("textbox", { name: "Autofocus field" }),
      ).toHaveFocus();
    });
  });

  it("traps focus within the dialog on Tab and Shift+Tab", async () => {
    const user = userEvent.setup();

    renderIntoRoot(
      <>
        <button>Outside before</button>
        <DialogOverlay>
          <DialogPanel labelledBy="trap-dialog-title">
            <h2 id="trap-dialog-title">Trap dialog</h2>
            <AutoFocusFirstButton
              firstLabel="First action"
              secondLabel="Second action"
            />
          </DialogPanel>
        </DialogOverlay>
        <button>Outside after</button>
      </>,
    );

    const firstButton = screen.getByRole("button", { name: "First action" });
    const secondButton = screen.getByRole("button", { name: "Second action" });

    await waitFor(() => {
      expect(firstButton).toHaveFocus();
    });

    await user.tab();
    expect(secondButton).toHaveFocus();

    await user.tab();
    expect(firstButton).toHaveFocus();

    await user.tab({ shift: true });
    expect(secondButton).toHaveFocus();
  });

  it("restores focus to the previously focused element on unmount", async () => {
    const user = userEvent.setup();

    function Harness(): React.ReactElement {
      const [isOpen, setIsOpen] = useState(false);

      return (
        <>
          <button>Previous target</button>
          <button
            onMouseDown={(event) => {
              event.preventDefault();
            }}
            onClick={() => {
              setIsOpen(true);
            }}
          >
            Open dialog
          </button>
          {isOpen ? (
            <DialogOverlay>
              <DialogPanel labelledBy="restore-focus-dialog-title">
                <h2 id="restore-focus-dialog-title">Restore focus dialog</h2>
                <button
                  onClick={() => {
                    setIsOpen(false);
                  }}
                >
                  Close dialog
                </button>
              </DialogPanel>
            </DialogOverlay>
          ) : null}
        </>
      );
    }

    renderIntoRoot(<Harness />);

    const previousTarget = screen.getByRole("button", {
      name: "Previous target",
    });
    previousTarget.focus();
    expect(previousTarget).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Open dialog" }));
    await user.click(screen.getByRole("button", { name: "Close dialog" }));

    await waitFor(() => {
      expect(previousTarget).toHaveFocus();
    });
  });

  it("restores focus to the previous modal when nested overlays close", async () => {
    const setNestedOpenReference: {
      current: ((isNestedOpen: boolean) => void) | undefined;
    } = {
      current: undefined,
    };

    function Harness(): React.ReactElement {
      const [isNestedOpen, updateNestedOpen] = useState(false);

      useEffect(() => {
        setNestedOpenReference.current = updateNestedOpen;
      }, [updateNestedOpen]);

      return (
        <DialogOverlay>
          <DialogPanel labelledBy="primary-dialog-title">
            <h2 id="primary-dialog-title">Primary dialog</h2>
            <button>Open nested dialog</button>
            {isNestedOpen ? (
              <DialogOverlay>
                <DialogPanel labelledBy="nested-dialog-title">
                  <h2 id="nested-dialog-title">Nested dialog</h2>
                  <AutoFocusCloseButton onClose={doNothing} />
                </DialogPanel>
              </DialogOverlay>
            ) : null}
          </DialogPanel>
        </DialogOverlay>
      );
    }

    const { root } = renderIntoRoot(<Harness />);
    const openNestedButton = screen.getByRole("button", {
      name: "Open nested dialog",
    });

    openNestedButton.focus();
    expect(openNestedButton).toHaveFocus();
    await waitFor(() => {
      expect(setNestedOpenReference.current).toBeDefined();
    });
    act(() => {
      setNestedOpenReference.current?.(true);
    });

    const closeNestedButton = screen.getByRole("button", {
      name: "Close nested",
    });

    await waitFor(() => {
      expect(closeNestedButton).toHaveFocus();
    });

    act(() => {
      setNestedOpenReference.current?.(false);
    });

    await waitFor(() => {
      expect(openNestedButton).toHaveFocus();
    });

    expectRootHidden(root, true);
  });

  it("keeps stack balanced under mount / unmount / mount cycles (StrictMode shape)", async () => {
    const user = userEvent.setup();

    function Harness(): React.ReactElement {
      const [isOpen, setIsOpen] = useState(true);

      return (
        <>
          <button>Outside trigger</button>
          {isOpen ? (
            <DialogOverlay>
              <DialogPanel labelledBy="strict-dialog-title">
                <h2 id="strict-dialog-title">Strict dialog</h2>
                <button
                  onClick={() => {
                    setIsOpen(false);
                  }}
                >
                  Close dialog
                </button>
              </DialogPanel>
            </DialogOverlay>
          ) : null}
        </>
      );
    }

    const { root } = renderIntoRoot(
      <StrictMode>
        <Harness />
      </StrictMode>,
    );

    expect(
      screen.getByRole("dialog", { name: "Strict dialog" }),
    ).toBeInTheDocument();
    expectRootHidden(root, true);

    await user.click(screen.getByRole("button", { name: "Close dialog" }));

    await waitFor(() => {
      expect(
        screen.queryByRole("dialog", { name: "Strict dialog" }),
      ).not.toBeInTheDocument();
    });

    expectRootHidden(root, false);
  });

  it("does not throw when the overlay has no descendant role=dialog element", () => {
    renderIntoRoot(
      <DialogOverlay>
        <div>No dialog root</div>
      </DialogOverlay>,
    );

    expect(screen.getByText("No dialog root")).toBeInTheDocument();
  });

  it("skips inert management without throwing when #root is absent", () => {
    document.body.innerHTML = "";

    render(
      <DialogOverlay>
        <DialogPanel labelledBy="no-root-dialog-title">
          <h2 id="no-root-dialog-title">No root dialog</h2>
        </DialogPanel>
      </DialogOverlay>,
    );

    expect(document.getElementById("root")).toBeNull();
    expect(
      screen.getByRole("dialog", { name: "No root dialog" }),
    ).toBeInTheDocument();
  });
});
