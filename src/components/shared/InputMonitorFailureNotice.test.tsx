import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { InputMonitorFailureNotice } from "./InputMonitorFailureNotice";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

let failureNotification: EventCallback<unknown> | undefined = undefined;

async function renderNotice(): Promise<void> {
  await act(async () => {
    render(<InputMonitorFailureNotice />);
    await Promise.resolve();
  });
}

function notifyFailure(): void {
  if (!failureNotification) throw new Error("failure subscription missing");
  vi.mocked(invoke).mockResolvedValue(true);
  failureNotification({ event: "input_monitor_failed", id: 1, payload: null });
}

describe("input monitoring failure notice", () => {
  beforeEach(() => {
    failureNotification = undefined;
    vi.mocked(invoke)
      .mockReset()
      .mockImplementation((command) => {
        if (command !== "has_input_monitor_failed")
          throw new Error("unexpected command");
        return Promise.resolve(false);
      });
    vi.mocked(listen)
      .mockReset()
      .mockImplementation((event, callback) => {
        if (event !== "input_monitor_failed")
          throw new Error("unexpected event");
        failureNotification = callback;
        return Promise.resolve((): void => undefined);
      });
  });

  it("does not report a monitoring failure when none was observed", async () => {
    await renderNotice();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a previously detected failure and both affected functions", async () => {
    vi.mocked(invoke).mockResolvedValue(true);
    await renderNotice();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "入力監視でエラーが発生しました",
    );
    expect(screen.getByRole("alert")).toHaveTextContent("ペナルティ");
    expect(screen.getByRole("alert")).toHaveTextContent("緊急解除キー");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "働かない可能性があります",
    );
  });

  it("notifies the user when monitoring fails after the screen opens", async () => {
    await renderNotice();
    act(notifyFailure);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力監視でエラーが発生しました",
    );
  });

  it("does not erase a new failure with an older snapshot", async () => {
    let resolveSnapshot: (value: boolean) => void = () => {
      throw new Error("snapshot not requested");
    };
    vi.mocked(invoke).mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveSnapshot = resolve;
      }),
    );
    await renderNotice();
    act(notifyFailure);
    await act(async () => {
      resolveSnapshot(false);
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "入力監視でエラーが発生しました",
    );
  });

  it("distinguishes a failed information request and lets the user retry", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("private backend detail"));
    await renderNotice();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "入力監視のエラー情報を取得できませんでした",
    );
    expect(
      screen.queryByText(/private backend detail/u),
    ).not.toBeInTheDocument();
    vi.mocked(invoke).mockResolvedValue(true);
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "再確認" }));
    expect(
      await screen.findByText("入力監視でエラーが発生しました。"),
    ).toBeInTheDocument();
  });

  it("reports notification connection failure without claiming monitoring failed", async () => {
    vi.mocked(listen).mockRejectedValue(new Error("offline"));
    await renderNotice();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "入力監視のエラー情報を取得できませんでした",
    );
    expect(
      screen.queryByText("入力監視でエラーが発生しました。"),
    ).not.toBeInTheDocument();
  });

  it("shows a recorded monitoring failure even when live notifications are unavailable", async () => {
    vi.mocked(listen).mockRejectedValue(new Error("offline"));
    vi.mocked(invoke).mockResolvedValue(true);
    await renderNotice();
    expect(
      await screen.findByText("入力監視でエラーが発生しました。"),
    ).toBeInTheDocument();
  });
});
