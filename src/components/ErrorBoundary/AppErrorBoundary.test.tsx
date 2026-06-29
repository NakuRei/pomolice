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
import { AppErrorBoundary } from "./AppErrorBoundary";

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

const ThrowingAppContent = throwOnRender("app crash");

let shouldCrash = false;

function RetryableAppContent(): React.ReactElement {
  if (shouldCrash) {
    throw new Error("app crash");
  }

  return <div>Recovered app content</div>;
}

describe("AppErrorBoundary", () => {
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
      <AppErrorBoundary>
        <div>App content</div>
      </AppErrorBoundary>,
    );

    expect(screen.getByText("App content")).toBeInTheDocument();
    expect(errorLoggerMock).not.toHaveBeenCalled();
    expect(infoLoggerMock).not.toHaveBeenCalled();
  });

  it("shows the app fallback and logs the caught render error", async () => {
    render(
      <>
        <AppErrorBoundary>
          <ThrowingAppContent />
        </AppErrorBoundary>
        <div>Outside content</div>
      </>,
    );

    expect(
      await screen.findByText("アプリケーションエラーが発生しました"),
    ).toBeInTheDocument();
    expect(screen.getByText("Outside content")).toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledTimes(1);
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({
        boundary_scope: "app",
      }),
    );
    const caughtPayload = errorLoggerMock.mock.calls[0]?.[2] as Record<
      string,
      unknown
    >;
    expect(caughtPayload.failure_message).toEqual(
      expect.stringContaining("app crash"),
    );
    expect(caughtPayload).toHaveProperty("component_stack");
    expect(caughtPayload).not.toHaveProperty("current_screen");
  });

  it("keeps the app fallback when retrying while children still throw", async () => {
    const user = userEvent.setup();
    shouldCrash = true;

    render(
      <AppErrorBoundary>
        <RetryableAppContent />
      </AppErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    await user.click(within(alert).getByRole("button"));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(screen.queryByText("Recovered app content")).not.toBeInTheDocument();
  });

  it("logs reset and redraws children when retry succeeds", async () => {
    const user = userEvent.setup();
    shouldCrash = true;

    render(
      <AppErrorBoundary>
        <RetryableAppContent />
      </AppErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    shouldCrash = false;
    await user.click(within(alert).getByRole("button"));

    expect(
      await screen.findByText("Recovered app content"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledTimes(1);
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "app" },
    );
  });
});
