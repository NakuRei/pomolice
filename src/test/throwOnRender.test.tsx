import { render, screen } from "@testing-library/react";
import { ErrorBoundary } from "react-error-boundary";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { throwOnRender } from "./throwOnRender";

describe("throwOnRender", () => {
  beforeAll(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  it("throws the given message so an ancestor error boundary catches it", () => {
    const Crash = throwOnRender("boom under boundary");

    render(
      <ErrorBoundary
        fallbackRender={({ error }): React.ReactElement => (
          <div>caught: {(error as Error).message}</div>
        )}
      >
        <Crash />
      </ErrorBoundary>,
    );

    expect(screen.getByText("caught: boom under boundary")).toBeInTheDocument();
  });

  it("ignores any props it receives and still throws the given message", () => {
    const Crash = throwOnRender("boom with props");

    render(
      <ErrorBoundary
        fallbackRender={({ error }): React.ReactElement => (
          <div>caught: {(error as Error).message}</div>
        )}
      >
        <Crash onClose={(): void => undefined} label="unused" count={3} />
      </ErrorBoundary>,
    );

    expect(screen.getByText("caught: boom with props")).toBeInTheDocument();
  });
});
