import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SkipFlowModal from "./SkipFlowModal";
import { SKIP_FLOW_STEPS, useSkipFlowStore } from "@/store/skipFlowStore";

const SKIP_FLOW_MODAL_NAME = "休憩スキップ確認";
const SKIP_FLOW_MODAL_DESCRIPTION =
  "休憩をスキップするには、この確認フローを順番に完了してください。各ステップの指示に従って進めてください。";

function createRootFixture(): HTMLDivElement {
  document.body.innerHTML = "";

  const root = document.createElement("div");
  root.id = "root";
  document.body.appendChild(root);

  return root;
}

function renderSkipFlowModal(currentStep: number): ReturnType<typeof render> & {
  root: HTMLDivElement;
  onComplete: ReturnType<typeof vi.fn>;
  onCancel: ReturnType<typeof vi.fn>;
} {
  const root = createRootFixture();
  const onComplete = vi.fn();
  const onCancel = vi.fn();

  useSkipFlowStore.getState().resetSkipFlow();
  useSkipFlowStore.setState({
    isActive: true,
    currentStep,
    startedAt: Date.now(),
  });

  const renderResult = render(
    <SkipFlowModal onComplete={onComplete} onCancel={onCancel} />,
    {
      container: root,
    },
  );

  return Object.assign(renderResult, {
    root,
    onComplete,
    onCancel,
  });
}

function getSkipFlowDialog(): HTMLElement {
  return screen.getByRole("dialog", { name: SKIP_FLOW_MODAL_NAME });
}

beforeEach(() => {
  document.body.innerHTML = "";
  useSkipFlowStore.getState().resetSkipFlow();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SkipFlowModal", () => {
  it("renders SkipFlowModal as a named modal dialog with aria-modal and the fixed modal title", () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    const dialog = getSkipFlowDialog();

    expect(dialog).toHaveAttribute("aria-modal", "true");
  });

  it("exposes the modal description via aria-describedby", () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    expect(getSkipFlowDialog()).toHaveAccessibleDescription(
      SKIP_FLOW_MODAL_DESCRIPTION,
    );
  });

  it("keeps focus inside the modal for a confirmation step", async () => {
    const user = userEvent.setup();
    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    const noTakeBreakButton = screen.getByRole("button", {
      name: "No, Take Break",
    });
    const yesContinueButton = screen.getByRole("button", {
      name: "Yes, Continue",
    });

    await waitFor(() => {
      expect(noTakeBreakButton).toHaveFocus();
    });

    await user.tab();
    expect(yesContinueButton).toHaveFocus();

    await user.tab();
    expect(noTakeBreakButton).toHaveFocus();
  });

  it("focuses the No Take Break button on confirmation step 1", async () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "No, Take Break" }),
      ).toHaveFocus();
    });
  });

  it("moves focus back to the No Take Break button when confirmation step 1 advances to step 2", async () => {
    const user = userEvent.setup();
    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    await user.click(screen.getByRole("button", { name: "Yes, Continue" }));

    await waitFor(() => {
      expect(useSkipFlowStore.getState().currentStep).toBe(
        SKIP_FLOW_STEPS.CONFIRMATION_2,
      );
      expect(
        screen.getByRole("button", { name: "No, Take Break" }),
      ).toHaveFocus();
    });
  });

  it("updates time spent once per second while the skip flow is active", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));

    renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    expect(useSkipFlowStore.getState().totalTimeSpent).toBe(0);

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(useSkipFlowStore.getState().totalTimeSpent).toBe(1);
  });

  it("cancels the skip flow before notifying the caller", async () => {
    const user = userEvent.setup();
    const { onCancel } = renderSkipFlowModal(SKIP_FLOW_STEPS.CONFIRMATION_1);

    onCancel.mockImplementation(() => {
      expect(useSkipFlowStore.getState().isActive).toBe(false);
    });

    await user.click(screen.getByRole("button", { name: "No, Take Break" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("completes the skip flow before notifying the caller", async () => {
    const user = userEvent.setup();
    const { onComplete } = renderSkipFlowModal(SKIP_FLOW_STEPS.FINAL_WARNING);

    onComplete.mockImplementation(() => {
      const state = useSkipFlowStore.getState();
      expect(state.isActive).toBe(false);
      expect(state.stepsCompleted[SKIP_FLOW_STEPS.FINAL_WARNING - 1]).toBe(
        true,
      );
    });

    await user.click(screen.getByRole("button", { name: "Yes, Skip Anyway" }));

    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("focuses the No Take the Break button when the final warning step is shown", async () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.FINAL_WARNING);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "No, Take the Break" }),
      ).toHaveFocus();
    });
  });

  it("focuses the press-and-hold button when the long-press step is shown", async () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.LONG_PRESS);

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Press and Hold" }),
      ).toHaveFocus();
    });
  });

  it("preserves TypingChallenge input autofocus when the typing step is rendered", async () => {
    renderSkipFlowModal(SKIP_FLOW_STEPS.TYPING_CHALLENGE_1);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Start typing here...")).toHaveFocus();
    });
  });
});
