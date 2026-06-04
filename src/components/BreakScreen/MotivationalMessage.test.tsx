import { afterEach, describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MotivationalMessage } from "./MotivationalMessage";
import * as motivationalMessagesModule from "@/data/motivationalMessages";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MotivationalMessage", () => {
  it("shows one of the configured motivational messages", () => {
    render(<MotivationalMessage />);

    const visibleMessages =
      motivationalMessagesModule.motivationalMessages.filter((message) =>
        screen.queryByText(message),
      );
    expect(visibleMessages).toHaveLength(1);
  });

  it("keeps the initially selected message across rerenders", () => {
    vi.spyOn(motivationalMessagesModule, "getRandomMessage")
      .mockReturnValueOnce("最初のメッセージ")
      .mockReturnValueOnce("次のメッセージ");

    const { rerender } = render(<MotivationalMessage />);

    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();

    rerender(<MotivationalMessage />);

    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();
    expect(screen.queryByText("次のメッセージ")).not.toBeInTheDocument();
  });
});
