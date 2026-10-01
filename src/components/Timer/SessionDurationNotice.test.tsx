import { useTimerObservationStore } from "@/store/timerObservationStore";
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";
import { SessionDurationNotice } from "./SessionDurationNotice";

const NOTICE_TEST_ID = "session-duration-notice";

describe("SessionDurationNotice", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
  });

  it("shows the running length and the next length when a save changed it", () => {
    useTimerObservationStore.setState({
      status: "working",
      sessionType: "work",
      sessionDurationSeconds: 25 * 60,
      settings: createSettingsForTest({ workDurationSecs: 30 * 60 }),
    });

    render(<SessionDurationNotice />);

    expect(screen.getByTestId(NOTICE_TEST_ID)).toHaveTextContent(
      "このセッションは25分・次回から30分",
    );
  });

  it("compares a break with the saved length of that kind of break", () => {
    useTimerObservationStore.setState({
      status: "on_break",
      sessionType: "long_break",
      sessionDurationSeconds: 15 * 60,
      settings: createSettingsForTest({ longBreakDurationSecs: 20 * 60 }),
    });

    render(<SessionDurationNotice />);

    expect(screen.getByTestId(NOTICE_TEST_ID)).toHaveTextContent(
      "このセッションは15分・次回から20分",
    );
  });

  it("shows nothing when the running session already has the saved length", () => {
    useTimerObservationStore.setState({
      status: "working",
      sessionType: "work",
      sessionDurationSeconds: 25 * 60,
      settings: createSettingsForTest({ workDurationSecs: 25 * 60 }),
    });

    render(<SessionDurationNotice />);

    expect(screen.queryByTestId(NOTICE_TEST_ID)).not.toBeInTheDocument();
  });

  it("shows nothing while idle", () => {
    useTimerObservationStore.setState({
      status: "idle",
      sessionDurationSeconds: 25 * 60,
      settings: createSettingsForTest({ workDurationSecs: 30 * 60 }),
    });

    render(<SessionDurationNotice />);

    expect(screen.queryByTestId(NOTICE_TEST_ID)).not.toBeInTheDocument();
  });

  it("shows nothing before the settings have arrived", () => {
    useTimerObservationStore.setState({
      status: "working",
      sessionDurationSeconds: 25 * 60,
      settings: null,
    });

    render(<SessionDurationNotice />);

    expect(screen.queryByTestId(NOTICE_TEST_ID)).not.toBeInTheDocument();
  });
});
