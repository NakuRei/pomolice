import { useTimerObservationStore } from "@/store/timerObservationStore";
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { SettingsFileNotice } from "./SettingsFileNotice";

describe("SettingsFileNotice", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
  });

  it("tells the user the saved settings could not be read", () => {
    useTimerObservationStore.setState({ settingsFileStatus: "unusable" });

    render(<SettingsFileNotice />);

    expect(screen.getByRole("status")).toHaveTextContent(
      "保存済みの設定を読み込めなかったため、初期設定で動作しています。",
    );
  });

  it("shows nothing while the settings file is usable", () => {
    useTimerObservationStore.setState({ settingsFileStatus: "ok" });

    render(<SettingsFileNotice />);

    expect(
      screen.queryByTestId("settings-file-notice"),
    ).not.toBeInTheDocument();
  });
});
