import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { useSettingsStore } from "./store/settingsStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    status: "idle",
    remaining_seconds: 1500,
    session_type: "work",
    current_cycle: 0,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    is_fullscreen_break: false,
    show_continue_dialog: false,
    settings: {
      work_duration_secs: 1500,
      short_break_duration_secs: 300,
      long_break_duration_secs: 900,
      long_break_interval: 4,
    },
  }),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

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

  it("toggles notification mute state when mute button is clicked", async () => {
    const user = userEvent.setup();
    useSettingsStore.setState({ notificationSoundMuted: false });

    render(<App />);

    const muteButton = screen.getByTestId("notification-mute-button");
    await user.click(muteButton);

    expect(useSettingsStore.getState().notificationSoundMuted).toBe(true);
  });
});
