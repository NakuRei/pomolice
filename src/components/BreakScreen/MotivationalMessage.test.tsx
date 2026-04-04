import { afterEach, describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MotivationalMessage } from "./MotivationalMessage";
import * as motivationalMessagesModule from "@/data/motivationalMessages";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MotivationalMessage", () => {
  it("renders the message selected by the message source", () => {
    vi.spyOn(motivationalMessagesModule, "getRandomMessage").mockReturnValue(
      "テストメッセージ",
    );

    render(<MotivationalMessage />);

    expect(screen.getByText("テストメッセージ")).toBeInTheDocument();
  });
});
