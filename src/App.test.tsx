import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders without crashing", () => {
    render(<App />);
  });

  it("displays 'Pomolice' title", () => {
    render(<App />);
    const heading = screen.getByText(/^Pomolice$/i);
    expect(heading).toBeInTheDocument();
  });

  it("renders timer display", () => {
    render(<App />);
    const timerDisplay = screen.getByTestId("timer-display");
    expect(timerDisplay).toBeInTheDocument();
  });

  it("renders start button", () => {
    render(<App />);
    const startButton = screen.getByTestId("start-button");
    expect(startButton).toBeInTheDocument();
  });
});
