import type { EmergencyShortcutStatus } from "@/ipc/generated";
import { act, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { beforeEach, expect, it, vi } from "vitest";

import {
  EmergencyShortcutHint,
  EmergencyShortcutNotice,
} from "./EmergencyShortcutNotice";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

let notification: EventCallback<EmergencyShortcutStatus> | undefined =
  undefined;

beforeEach(() => {
  notification = undefined;
  vi.mocked(invoke)
    .mockReset()
    .mockResolvedValue({ status: "accepted", value: { kind: "registering" } });
  vi.mocked(listen)
    .mockReset()
    .mockImplementation((event, callback) => {
      if (event !== "emergency_shortcut_changed")
        throw new Error("unexpected event");
      notification = callback;
      return Promise.resolve(() => undefined);
    });
});

async function openScreen(): Promise<void> {
  await act(async () => {
    render(
      <>
        <EmergencyShortcutNotice />
        <EmergencyShortcutHint />
      </>,
    );
    await Promise.resolve();
  });
}

function notify(status: EmergencyShortcutStatus): void {
  if (!notification) throw new Error("missing subscription");
  notification({ event: "emergency_shortcut_changed", id: 1, payload: status });
}

it("does not advertise the preferred shortcut while awaiting permission", async () => {
  await openScreen();
  expect(screen.getByText(/緊急解除：登録待ち/u)).toBeInTheDocument();
  expect(screen.queryByText(/Ctrl/u)).not.toBeInTheDocument();
});

it("shows the assigned shortcut and follows changes made in desktop settings", async () => {
  vi.mocked(invoke).mockResolvedValue({
    status: "accepted",
    value: {
      kind: "registered",
      trigger: "Super + F12",
    },
  });
  await openScreen();
  expect(screen.getByText("Super + F12")).toBeInTheDocument();
  act(() => {
    notify({ kind: "registered", trigger: "Ctrl + F10" });
  });
  expect(screen.getByText("Ctrl + F10")).toBeInTheDocument();
  expect(screen.queryByText("Super + F12")).not.toBeInTheDocument();
});

it("removes unusable shortcut guidance and reports loss of availability", async () => {
  vi.mocked(invoke).mockResolvedValue({
    status: "accepted",
    value: {
      kind: "registered",
      trigger: "Super + F12",
    },
  });
  await openScreen();
  act(() => {
    notify({ kind: "unavailable" });
  });
  expect(screen.queryByText("Super + F12")).not.toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "緊急解除のショートカットを利用できません",
  );
});

it("shows the existing shortcut when the input monitor supplies emergency release", async () => {
  vi.mocked(invoke).mockResolvedValue({
    status: "accepted",
    value: { kind: "input_monitor" },
  });
  await openScreen();
  expect(screen.getByText("Ctrl + Alt + Shift + E")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("does not replace a new registration with an older snapshot", async () => {
  let resolveSnapshot: (status: EmergencyShortcutStatus) => void = () => {
    throw new Error("not requested");
  };
  vi.mocked(invoke).mockReturnValue(
    new Promise<EmergencyShortcutStatus>((resolve) => {
      resolveSnapshot = resolve;
    }),
  );
  await openScreen();
  act(() => {
    notify({ kind: "registered", trigger: "Super + F12" });
  });
  await act(async () => {
    resolveSnapshot({ kind: "registering" });
    await Promise.resolve();
  });
  expect(screen.getByText("Super + F12")).toBeInTheDocument();
});

it("reports an information failure without inventing an available shortcut", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("internal detail"));
  await openScreen();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "緊急解除のショートカット情報を取得できません",
  );
  expect(screen.getByText("緊急解除：確認不可")).toBeInTheDocument();
  expect(screen.queryByText(/Ctrl|internal detail/u)).not.toBeInTheDocument();
});
