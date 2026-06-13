import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  afterAll,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { FRONTEND_LOG_EVENT_IDS } from "../../logging/logEvents";
import { throwOnRender } from "../../test/throwOnRender";
import { FullscreenBreakErrorBoundary } from "./FullscreenBreakErrorBoundary";

const { errorLoggerMock, infoLoggerMock } = vi.hoisted(() => ({
  errorLoggerMock: vi.fn(),
  infoLoggerMock: vi.fn(),
}));

vi.mock("../../logging/logger", () => ({
  createLogger: vi.fn(() => ({
    debug: vi.fn(),
    error: errorLoggerMock,
    info: infoLoggerMock,
    trace: vi.fn(),
    warn: vi.fn(),
  })),
}));

const ThrowingBreakContent = throwOnRender("fullscreen crash");

let shouldCrash = false;

function RetryableBreakContent(): React.ReactElement {
  if (shouldCrash) {
    throw new Error("fullscreen crash");
  }

  return <div>Recovered break content</div>;
}

describe("FullscreenBreakErrorBoundary", () => {
  beforeAll(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    shouldCrash = false;
    errorLoggerMock.mockClear();
    infoLoggerMock.mockClear();
  });

  it("renders children while they do not throw", () => {
    render(
      <FullscreenBreakErrorBoundary>
        <div>Break content</div>
      </FullscreenBreakErrorBoundary>,
    );

    expect(screen.getByText("Break content")).toBeInTheDocument();
    expect(errorLoggerMock).not.toHaveBeenCalled();
    expect(infoLoggerMock).not.toHaveBeenCalled();
  });

  it("shows the fullscreen fallback and logs the caught render error", async () => {
    render(
      <>
        <FullscreenBreakErrorBoundary>
          <ThrowingBreakContent />
        </FullscreenBreakErrorBoundary>
        <div>Outside content</div>
      </>,
    );

    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("Outside content")).toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledTimes(1);
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({
        boundary_scope: "fullscreen_break",
      }),
    );
    const caughtPayload = errorLoggerMock.mock.calls[0]?.[2] as Record<
      string,
      unknown
    >;
    expect(caughtPayload.failure_message).toEqual(
      expect.stringContaining("fullscreen crash"),
    );
    expect(caughtPayload).toHaveProperty("component_stack");
    expect(caughtPayload).not.toHaveProperty("current_screen");
  });

  it("keeps the fullscreen fallback when retrying while children still throw", async () => {
    const user = userEvent.setup();
    shouldCrash = true;

    render(
      <FullscreenBreakErrorBoundary>
        <RetryableBreakContent />
      </FullscreenBreakErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    await user.click(within(alert).getByRole("button"));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(
      screen.queryByText("Recovered break content"),
    ).not.toBeInTheDocument();
  });

  it("logs reset and redraws children when retry succeeds", async () => {
    const user = userEvent.setup();
    shouldCrash = true;

    render(
      <FullscreenBreakErrorBoundary>
        <RetryableBreakContent />
      </FullscreenBreakErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    shouldCrash = false;
    await user.click(within(alert).getByRole("button"));

    expect(
      await screen.findByText("Recovered break content"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledTimes(1);
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "fullscreen_break" },
    );
  });
});
