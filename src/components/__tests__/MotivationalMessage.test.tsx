import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MotivationalMessage } from "../BreakScreen/MotivationalMessage";
import * as motivationalMessagesModule from "@/data/motivationalMessages";

describe("MotivationalMessage", () => {
  describe("Rendering", () => {
    it("should render a motivational message", () => {
      render(<MotivationalMessage />);

      // Should render some text (any message from the array)
      const messageElement = screen.getByText(/./u);
      expect(messageElement).toBeInTheDocument();
    });
  });

  describe("Message Selection", () => {
    it("should call getRandomMessage on mount", () => {
      const getRandomMessageSpy = vi.spyOn(
        motivationalMessagesModule,
        "getRandomMessage",
      );

      render(<MotivationalMessage />);

      expect(getRandomMessageSpy).toHaveBeenCalledTimes(1);
    });

    it("should display the message returned by getRandomMessage", () => {
      const testMessage = "テストメッセージ 🎯";
      vi.spyOn(motivationalMessagesModule, "getRandomMessage").mockReturnValue(
        testMessage,
      );

      render(<MotivationalMessage />);

      expect(screen.getByText(testMessage)).toBeInTheDocument();
    });

    it("should display different messages on different renders", () => {
      const messages = ["メッセージ1", "メッセージ2", "メッセージ3"];

      let callCount = 0;
      vi.spyOn(
        motivationalMessagesModule,
        "getRandomMessage",
      ).mockImplementation(() => messages[callCount++ % messages.length]);

      const { unmount } = render(<MotivationalMessage />);
      expect(screen.getByText("メッセージ1")).toBeInTheDocument();

      unmount();

      render(<MotivationalMessage />);
      expect(screen.getByText("メッセージ2")).toBeInTheDocument();
    });
  });

  describe("Integration", () => {
    it("should handle empty message gracefully", () => {
      vi.spyOn(motivationalMessagesModule, "getRandomMessage").mockReturnValue(
        "",
      );

      render(<MotivationalMessage />);

      // Should not crash, container should exist
      const container = screen.getByText("", { selector: "p" });
      expect(container).toBeInTheDocument();
    });

    it("should handle long messages", () => {
      const longMessage = "これは非常に長いメッセージです。".repeat(10);
      vi.spyOn(motivationalMessagesModule, "getRandomMessage").mockReturnValue(
        longMessage,
      );

      render(<MotivationalMessage />);

      expect(screen.getByText(longMessage)).toBeInTheDocument();
    });
  });
});
