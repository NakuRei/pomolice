import { afterEach, describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MotivationalMessage } from "./MotivationalMessage";
import * as motivationalMessagesModule from "@/data/motivationalMessages";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MotivationalMessage", () => {
  it("keeps the initially selected message across rerenders", () => {
    const getRandomMessage = vi
      .spyOn(motivationalMessagesModule, "getRandomMessage")
      .mockReturnValueOnce("最初のメッセージ")
      .mockReturnValueOnce("次のメッセージ");

    const { rerender } = render(<MotivationalMessage />);

    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();

    rerender(<MotivationalMessage />);

    expect(getRandomMessage).toHaveBeenCalledTimes(1);
    expect(screen.getByText("最初のメッセージ")).toBeInTheDocument();
    expect(screen.queryByText("次のメッセージ")).not.toBeInTheDocument();
  });
});
