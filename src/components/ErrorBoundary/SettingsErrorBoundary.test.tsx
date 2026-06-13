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
import { SettingsErrorBoundary } from "./SettingsErrorBoundary";

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

const ThrowingSettingsContent = throwOnRender("settings crash");

let shouldCrash = false;

function RetryableSettingsContent(): React.ReactElement {
  if (shouldCrash) {
    throw new Error("settings crash");
  }

  return <div>Recovered settings content</div>;
}

describe("SettingsErrorBoundary", () => {
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
      <SettingsErrorBoundary
        currentScreen="settingsTimer"
        onBackToHome={vi.fn()}
      >
        <div>Settings content</div>
      </SettingsErrorBoundary>,
    );

    expect(screen.getByText("Settings content")).toBeInTheDocument();
    expect(errorLoggerMock).not.toHaveBeenCalled();
    expect(infoLoggerMock).not.toHaveBeenCalled();
  });

  it("shows the settings fallback and logs the caught render error", async () => {
    render(
      <>
        <SettingsErrorBoundary
          currentScreen="settingsTimer"
          onBackToHome={vi.fn()}
        >
          <ThrowingSettingsContent />
        </SettingsErrorBoundary>
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
        boundary_scope: "settings",
        current_screen: "settingsTimer",
      }),
    );
    const caughtPayload = errorLoggerMock.mock.calls[0]?.[2] as Record<
      string,
      unknown
    >;
    expect(caughtPayload.failure_message).toEqual(
      expect.stringContaining("settings crash"),
    );
    expect(caughtPayload).toHaveProperty("component_stack");
  });

  it("calls the home action when the fallback home button is clicked", async () => {
    const user = userEvent.setup();
    const onBackToHome = vi.fn();

    render(
      <SettingsErrorBoundary
        currentScreen="settingsTimer"
        onBackToHome={onBackToHome}
      >
        <ThrowingSettingsContent />
      </SettingsErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    await user.click(
      within(alert).getByRole("button", {
        name: /ホーム/u,
      }),
    );

    expect(onBackToHome).toHaveBeenCalledTimes(1);
  });

  it("keeps the settings fallback when retrying while children still throw", async () => {
    const user = userEvent.setup();
    shouldCrash = true;
    render(
      <SettingsErrorBoundary
        currentScreen="settingsTimer"
        onBackToHome={vi.fn()}
      >
        <RetryableSettingsContent />
      </SettingsErrorBoundary>,
    );

    const alert = await screen.findByRole("alert");

    await user.click(
      within(alert).getByRole("button", {
        name: /再表示/u,
      }),
    );

    await waitFor(() => {
      expect(screen.getByRole("alert")).toBeInTheDocument();
    });
    expect(
      screen.queryByText("Recovered settings content"),
    ).not.toBeInTheDocument();
  });

  it("logs reset and redraws children when the current settings screen changes", async () => {
    shouldCrash = true;
    const { rerender } = render(
      <SettingsErrorBoundary
        currentScreen="settingsTimer"
        onBackToHome={vi.fn()}
      >
        <RetryableSettingsContent />
      </SettingsErrorBoundary>,
    );

    expect(await screen.findByRole("alert")).toBeInTheDocument();

    shouldCrash = false;
    rerender(
      <SettingsErrorBoundary currentScreen="settingsTop" onBackToHome={vi.fn()}>
        <RetryableSettingsContent />
      </SettingsErrorBoundary>,
    );

    expect(
      await screen.findByText("Recovered settings content"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(infoLoggerMock).toHaveBeenCalledTimes(1);
    });
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });
});
