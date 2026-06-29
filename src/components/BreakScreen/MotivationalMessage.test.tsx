import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MotivationalMessage } from "./MotivationalMessage";

describe("MotivationalMessage", () => {
  it("shows the given motivational message", () => {
    render(<MotivationalMessage message="水分補給の時間です" />);

    expect(screen.getByText("水分補給の時間です")).toBeInTheDocument();
  });
});
