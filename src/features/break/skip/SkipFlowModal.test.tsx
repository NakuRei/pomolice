import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SkipFlowModal from "./SkipFlowModal";
import { useSkipFlowStore } from "@/features/break/skipFlowStore";
import { SHORT_TYPING_PHRASES, LONG_TYPING_PHRASES } from "./typingPhrases";

const SKIP_FLOW_MODAL_NAME = "休憩スキップ確認";
const SKIP_FLOW_MODAL_DESCRIPTION =
  "休憩をスキップするには、この確認フローを順番に完了してください。各ステップの指示に従って進めてください。";
const REQUIRED_SKIP_FLOW_STEPS = 10;

function createRootFixture(): HTMLDivElement {
  document.body.innerHTML = "";

  const root = document.createElement("div");
  root.id = "root";
  document.body.appendChild(root);

  return root;
}

function renderSkipFlowModal(): ReturnType<typeof render> & {
  root: HTMLDivElement;
  onComplete: ReturnType<typeof vi.fn>;
  onCancel: ReturnType<typeof vi.fn>;
} {
  const root = createRootFixture();
  const onComplete = vi.fn();
  const onCancel = vi.fn();

  useSkipFlowStore.getState().resetSkipFlow();
  useSkipFlowStore.getState().startSkipFlow();

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

function expectSkipFlowStep(currentStep: number): void {
  expect(
    screen.getByText(
      `Step ${String(currentStep)} of ${String(REQUIRED_SKIP_FLOW_STEPS)}`,
    ),
  ).toBeInTheDocument();
}

function renderedPhraseFrom(bank: readonly string[]): string {
  const phrase = bank.find(
    (candidate) =>
      screen.queryAllByText((_, element) => element?.textContent === candidate)
        .length > 0,
  );

  if (phrase === undefined) {
    throw new Error("rendered phrase was not found in the expected bank");
  }

  return phrase;
}

function solveDisplayedProblem(): number {
  const text = screen.getByText(/[=]\s*\?/u).textContent;
  const groups = /(?<left>-?\d+)\s*(?<operator>[+\-×])\s*(?<right>-?\d+)/u.exec(
    text,
  )?.groups;

  if (groups?.operator === undefined) {
    throw new Error(`could not parse displayed problem: "${text}"`);
  }

  const left = Number(groups.left);
  const right = Number(groups.right);

  switch (groups.operator) {
    case "+":
      return left + right;
    case "×":
      return left * right;
    case "-":
      return left - right;
    default:
      throw new Error(`unexpected operator in problem: "${text}"`);
  }
}

function formElementOf(input: HTMLElement): HTMLFormElement {
  const form = input.closest("form");

  if (form === null) {
    throw new Error("answer input is not inside a form");
  }

  return form;
}

function continueFromConfirmationStep(): void {
  fireEvent.click(screen.getByRole("button", { name: "Yes, Continue" }));
}

function completeTypingChallenge(bank: readonly string[]): void {
  const phrase = renderedPhraseFrom(bank);
  fireEvent.change(screen.getByPlaceholderText("Start typing here..."), {
    target: { value: phrase },
  });
}

function completeMathChallenge(): void {
  const input = screen.getByPlaceholderText("Your answer");
  fireEvent.change(input, {
    target: { value: String(solveDisplayedProblem()) },
  });
  fireEvent.submit(formElementOf(input));
}

function completeLongPressChallenge(): void {
  const button = screen.getByRole("button", { name: /Press and Hold/iu });

  act(() => {
    fireEvent.mouseDown(button);
    vi.advanceTimersByTime(15100);
  });
}

beforeEach(() => {
  document.body.innerHTML = "";
  useSkipFlowStore.getState().resetSkipFlow();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SkipFlowModal", () => {
  it("renders a named modal dialog with aria-modal", () => {
    renderSkipFlowModal();

    const dialog = getSkipFlowDialog();

    expect(dialog).toHaveAttribute("aria-modal", "true");
  });

  it("exposes the modal description via aria-describedby", () => {
    renderSkipFlowModal();

    expect(getSkipFlowDialog()).toHaveAccessibleDescription(
      SKIP_FLOW_MODAL_DESCRIPTION,
    );
  });

  it("keeps focus inside the modal for a confirmation step", async () => {
    const user = userEvent.setup();
    renderSkipFlowModal();

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
    renderSkipFlowModal();

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "No, Take Break" }),
      ).toHaveFocus();
    });
  });

  it("moves focus back to the No Take Break button when confirmation step 1 advances to step 2", async () => {
    const user = userEvent.setup();
    renderSkipFlowModal();

    await user.click(screen.getByRole("button", { name: "Yes, Continue" }));

    await waitFor(() => {
      expectSkipFlowStep(2);
      expect(
        screen.getByRole("button", { name: "No, Take Break" }),
      ).toHaveFocus();
    });
  });

  it("notifies the caller when the user cancels the skip flow", async () => {
    const user = userEvent.setup();
    const { onCancel } = renderSkipFlowModal();

    await user.click(screen.getByRole("button", { name: "No, Take Break" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("requires the visible ten-step flow before completing the skip", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));

    const { onComplete } = renderSkipFlowModal();

    expectSkipFlowStep(1);
    expect(
      screen.getByText("Are you sure you want to skip this break?"),
    ).toBeInTheDocument();

    continueFromConfirmationStep();
    expectSkipFlowStep(2);
    expect(
      screen.getByText(
        "Breaks are important for your health. Do you still want to skip?",
      ),
    ).toBeInTheDocument();

    continueFromConfirmationStep();
    expectSkipFlowStep(3);
    expect(
      screen.getByText(
        "Your body needs rest. Are you certain you want to skip?",
      ),
    ).toBeInTheDocument();

    continueFromConfirmationStep();
    expectSkipFlowStep(4);
    expect(
      screen.getByText("This is your last warning. Really skip this break?"),
    ).toBeInTheDocument();

    continueFromConfirmationStep();
    expectSkipFlowStep(5);
    expect(screen.getByText("Typing Challenge")).toBeInTheDocument();

    completeTypingChallenge(SHORT_TYPING_PHRASES);
    expectSkipFlowStep(6);
    expect(screen.getByText("Math Challenge")).toBeInTheDocument();

    completeMathChallenge();
    expectSkipFlowStep(7);
    expect(screen.getByText("Typing Challenge")).toBeInTheDocument();

    completeTypingChallenge(LONG_TYPING_PHRASES);
    expectSkipFlowStep(8);
    expect(screen.getByText("Math Challenge")).toBeInTheDocument();

    completeMathChallenge();
    expectSkipFlowStep(9);
    expect(screen.getByText("Long Press Challenge")).toBeInTheDocument();

    completeLongPressChallenge();
    expectSkipFlowStep(10);
    expect(screen.getByText("Final Warning")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Yes, Skip Anyway" }));

    expect(onComplete).toHaveBeenCalledTimes(1);
  });
});
