import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import { useTimerEffectsStore } from "@/store/timerEffectsStore";
import { SHORT_TYPING_PHRASES } from "@/data/typingPhrases";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";
import { TimerEffectsNotice } from "../shared/TimerEffectsNotice";
import SkipFlowModal from "./SkipFlowModal";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function BreakView(): React.ReactElement {
  const isActive = useSkipFlowStore((state) => state.isActive);
  const resetSkipFlow = useSkipFlowStore((state) => state.resetSkipFlow);
  return (
    <>
      <TimerEffectsNotice />
      {isActive && (
        <SkipFlowModal onCancel={resetSkipFlow} onComplete={resetSkipFlow} />
      )}
    </>
  );
}

function renderBreakView(): HTMLDivElement {
  const root = document.createElement("div");
  root.id = "root";
  document.body.appendChild(root);
  useSkipFlowStore.getState().startSkipFlow();
  render(<BreakView />, { container: root });
  return root;
}

function observeSaveFailure(): void {
  act(() => {
    useTimerEffectsStore
      .getState()
      .observe(
        createTimerEffectsForTest({ revision: "1", last_timer_save: "failed" }),
      );
  });
}

function replyWithRecoveredSave(): void {
  vi.mocked(invoke).mockResolvedValue({
    status: "accepted",
    value: createTimerSnapshotForTest({
      effects: createTimerEffectsForTest({
        revision: "2",
        last_timer_save: "succeeded",
      }),
    }),
  });
}

describe("effect recovery during the skip flow", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    document.body.innerHTML = "";
    useSkipFlowStore.getState().resetSkipFlow();
    useTimerEffectsStore.setState(useTimerEffectsStore.getInitialState(), true);
  });

  it("allows keyboard recovery inside the modal without losing the step or typed text", async () => {
    const user = userEvent.setup();
    const root = renderBreakView();
    for (let step = 1; step <= 4; step += 1) {
      fireEvent.click(screen.getByRole("button", { name: "Yes, Continue" }));
    }
    const phrase = SHORT_TYPING_PHRASES.find(
      (candidate) =>
        screen.queryAllByText(
          (_, element) => element?.textContent === candidate,
        ).length > 0,
    );
    if (phrase === undefined) throw new Error("Typing phrase is missing");
    const partialText = phrase.slice(0, 3);
    const input = screen.getByPlaceholderText("Start typing here...");
    fireEvent.change(input, { target: { value: partialText } });

    observeSaveFailure();

    const dialog = screen.getByRole("dialog", { name: "休憩スキップ確認" });
    expect(root).toHaveAttribute("inert");
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      "保存に失敗しました",
    );
    expect(input).toHaveFocus();
    await user.tab({ shift: true });
    expect(
      within(dialog).getByRole("button", { name: "再試行" }),
    ).toHaveFocus();

    replyWithRecoveredSave();
    await user.keyboard("{Enter}");

    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    expect(
      screen.queryByRole("alert", { hidden: true }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("dialog", { name: "休憩スキップ確認" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Step 5 of 10")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Start typing here...")).toHaveValue(
      partialText,
    );
    await user.tab();
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
  });

  it("keeps an unresolved failure actionable in the main view after closing the modal", async () => {
    const user = userEvent.setup();
    const root = renderBreakView();
    observeSaveFailure();
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          revision: "1",
          last_timer_save: "failed",
        }),
      }),
    });

    await user.click(screen.getByRole("button", { name: "再試行" }));
    expect(screen.getByRole("alert")).toHaveTextContent("保存に失敗しました");
    await user.click(screen.getByRole("button", { name: "No, Take Break" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(root).not.toHaveAttribute("inert");
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("保存に失敗しました");
    replyWithRecoveredSave();
    await user.click(screen.getByRole("button", { name: "再試行" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
