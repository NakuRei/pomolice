import { beforeEach, describe, expect, it } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { ConnectionBanner } from "./ConnectionBanner";
import { useTimerSyncStore } from "@/store/timerSyncStore";
describe("ConnectionBanner", () => {
  beforeEach(() => {
    useTimerSyncStore.setState(useTimerSyncStore.getInitialState(), true);
  });
  it("shows no failure while idle or synchronized", () => {
    render(<ConnectionBanner />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    act(() => {
      useTimerSyncStore.getState().setConnection({ status: "ready" });
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("requests a new connection and keeps its progress owned by synchronization", () => {
    useTimerSyncStore.getState().setConnection({
      status: "failed",
      failure: { status: "unavailable", cause: "offline" },
    });
    render(<ConnectionBanner />);
    fireEvent.click(screen.getByRole("button", { name: "再試行" }));
    expect(useTimerSyncStore.getState().generation).toBe(1);
    expect(screen.getByRole("button", { name: "接続中" })).toBeDisabled();
    act(() => {
      useTimerSyncStore.getState().setConnection({ status: "ready" });
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("does not offer a retry for a nonretryable refusal", () => {
    useTimerSyncStore.getState().setConnection({
      status: "failed",
      failure: { status: "rejected", error: { code: "unauthorized_window" } },
    });
    render(<ConnectionBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent("権限エラー");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
