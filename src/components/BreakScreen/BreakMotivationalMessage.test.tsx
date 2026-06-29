import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { motivationalMessages } from "@/data/motivationalMessages";
import { BreakMotivationalMessage } from "./BreakMotivationalMessage";

function getVisibleMotivationalMessage(): string {
  const visibleMessages = motivationalMessages.filter(
    (message) => screen.queryByText(message) !== null,
  );

  expect(visibleMessages).toHaveLength(1);

  const visibleMessage = visibleMessages[0];
  if (visibleMessage === undefined) {
    throw new Error("Expected one motivational message to be visible.");
  }

  return visibleMessage;
}

describe("BreakMotivationalMessage", () => {
  it("shows one configured motivational message", () => {
    render(<BreakMotivationalMessage />);

    getVisibleMotivationalMessage();
  });

  it("keeps the initially selected message across rerenders", () => {
    let selectedMessage = "最初のメッセージ";
    const selectMessage = (): string => selectedMessage;
    const { rerender } = render(
      <BreakMotivationalMessage selectMessage={selectMessage} />,
    );

    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();

    selectedMessage = "次のメッセージ";
    rerender(<BreakMotivationalMessage selectMessage={selectMessage} />);

    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();
    expect(screen.queryByText("次のメッセージ")).not.toBeInTheDocument();
  });
});
