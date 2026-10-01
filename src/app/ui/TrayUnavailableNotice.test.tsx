import { useTraySync } from "@/app/sync/useTraySync";
import { useTrayStore } from "@/backend/trayStore";
import type { commands } from "@/ipc/generated";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TrayUnavailableNotice } from "./TrayUnavailableNotice";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const trayStatusRequest = vi.fn<typeof commands.isTrayAvailable>();
const exitRequest = vi.fn<typeof commands.exitWithoutTray>();

function SynchronizedNotice(): React.ReactElement {
  useTraySync();
  return <TrayUnavailableNotice />;
}

describe("tray initialization failure notice", () => {
  beforeEach(() => {
    useTrayStore.setState(useTrayStore.getInitialState(), true);
    trayStatusRequest
      .mockReset()
      .mockResolvedValue({ status: "accepted", value: false });
    exitRequest
      .mockReset()
      .mockResolvedValue({ status: "accepted", value: null });
    vi.mocked(invoke)
      .mockReset()
      .mockImplementation((command) => {
        switch (command) {
          case "is_tray_available":
            return trayStatusRequest();
          case "exit_without_tray":
            return exitRequest();
          default:
            return Promise.reject(
              new Error(`Unexpected IPC command: ${command}`),
            );
        }
      });
  });

  it("does not offer fallback exit when the tray is available", async () => {
    const statusResponse = Promise.resolve({
      status: "accepted",
      value: true,
    } as const);
    trayStatusRequest.mockReturnValue(statusResponse);
    await act(async () => {
      render(<SynchronizedNotice />);
      await statusResponse;
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "アプリを終了" }),
    ).not.toBeInTheDocument();
    expect(exitRequest).not.toHaveBeenCalled();
  });

  it("explains why the clock stays visible and offers explicit exit", async () => {
    render(<SynchronizedNotice />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "トレイを初期化できませんでした",
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "閉じるボタンを押しても時計は非表示になりません",
    );
    expect(screen.getByRole("button", { name: "アプリを終了" })).toBeEnabled();
    expect(exitRequest).not.toHaveBeenCalled();
  });

  it("requests application exit only after the user selects it", async () => {
    const user = userEvent.setup();
    render(<SynchronizedNotice />);
    await user.click(
      await screen.findByRole("button", { name: "アプリを終了" }),
    );
    expect(invoke).toHaveBeenCalledWith("exit_without_tray");
  });

  it("keeps the explanation and allows another attempt when exit fails", async () => {
    exitRequest.mockRejectedValue(new Error("private backend detail"));
    const user = userEvent.setup();
    render(<SynchronizedNotice />);
    await user.click(
      await screen.findByRole("button", { name: "アプリを終了" }),
    );
    expect(
      await screen.findByText("終了できませんでした。もう一度お試しください。"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/private backend detail/u),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "トレイを初期化できませんでした",
    );
    exitRequest
      .mockClear()
      .mockResolvedValue({ status: "accepted", value: null });
    await user.click(screen.getByRole("button", { name: "アプリを終了" }));
    expect(exitRequest).toHaveBeenCalled();
    expect(
      screen.queryByText("終了できませんでした。もう一度お試しください。"),
    ).not.toBeInTheDocument();
  });

  it("does not mistake a failed status request for tray initialization failure", async () => {
    trayStatusRequest.mockRejectedValue(new Error("offline"));
    const user = userEvent.setup();
    render(<SynchronizedNotice />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "トレイの状態を確認できませんでした",
    );
    expect(
      screen.queryByRole("button", { name: "アプリを終了" }),
    ).not.toBeInTheDocument();
    trayStatusRequest.mockResolvedValue({ status: "accepted", value: true });
    await user.click(screen.getByRole("button", { name: "再確認" }));
    await waitFor(() => {
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  it("offers fallback exit when a later status check confirms the tray is unavailable", async () => {
    trayStatusRequest.mockRejectedValue(new Error("offline"));
    const user = userEvent.setup();
    render(<SynchronizedNotice />);
    const retryButton = await screen.findByRole("button", { name: "再確認" });
    trayStatusRequest.mockResolvedValue({ status: "accepted", value: false });
    await user.click(retryButton);
    expect(
      await screen.findByRole("button", { name: "アプリを終了" }),
    ).toBeEnabled();
  });
});
