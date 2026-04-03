import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useFocusHeadingOnMount } from "./useFocusHeadingOnMount";

function HeadingHarness(): React.ReactElement {
  const headingRef = useFocusHeadingOnMount();

  return (
    <h1 ref={headingRef} tabIndex={-1}>
      Settings
    </h1>
  );
}

function UnattachedHeadingRefHarness(): React.ReactElement {
  useFocusHeadingOnMount();

  return <div>Settings</div>;
}

describe("useFocusHeadingOnMount", () => {
  it("returns a heading ref that focuses the mounted heading element", async () => {
    render(<HeadingHarness />);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Settings" })).toHaveFocus();
    });
  });

  it("does not throw when the ref is not attached to a heading element", () => {
    expect(() => render(<UnattachedHeadingRefHarness />)).not.toThrow();

    expect(screen.getByText("Settings")).toBeInTheDocument();
  });
});
