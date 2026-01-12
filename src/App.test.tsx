import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders without crashing", () => {
    render(<App />);
  });

  it("displays 'Hello Pomolice' message", () => {
    render(<App />);
    const heading = screen.getByText(/Hello Pomolice/i);
    expect(heading).toBeInTheDocument();
  });

  it("renders heading with correct styling classes", () => {
    render(<App />);
    const heading = screen.getByText(/Hello Pomolice/i);
    expect(heading).toHaveClass("text-4xl", "font-bold", "text-gray-800");
  });
});
