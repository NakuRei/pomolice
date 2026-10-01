import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  listen,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { AppRoot } from "./AppRoot";
import { AppSynchronization } from "./app/AppSynchronization";
import { EmergencyShortcutHint } from "@/features/break/ui/EmergencyShortcutHint";
import { EmergencyShortcutNotice } from "@/app/ui/EmergencyShortcutNotice";
import { useBackendObservationStore } from "./backend/observationStore";
import { useBackendSyncStore } from "./backend/syncStore";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createTimerSnapshotForTest } from "./test/timerEffectsFixture";
import type { SessionCompletedPayload } from "./ipc/generated";

const failure = vi.hoisted(() => ({ render: false }));

vi.mock("./App", () => ({
  default: function TestApp(): React.ReactElement {
    const remaining = useBackendObservationStore(
      (state) => state.remainingSeconds,
    );
    if (failure.render) throw new Error("screen failed");
    return <p>Remaining: {remaining}</p>;
  },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

const subscriptions = new Map<string, Set<EventCallback<unknown>>>();

function emit(event: string, payload: unknown): void {
  const callbacks = subscriptions.get(event);
  if (callbacks === undefined || callbacks.size === 0)
    throw new Error(`No subscription for ${event}`);
  callbacks.forEach((callback) => {
    callback({ event, id: 1, payload });
  });
}

beforeEach(() => {
  resetAppStateForTest();
  failure.render = false;
  subscriptions.clear();
  vi.mocked(sendNotification).mockReset();
  vi.mocked(isPermissionGranted).mockReset().mockResolvedValue(true);
  vi.mocked(invoke)
    .mockReset()
    .mockImplementation((command) => {
      switch (command) {
        case "get_full_state":
          return Promise.resolve({
            status: "accepted",
            value: createTimerSnapshotForTest(),
          });
        case "get_emergency_shortcut_status":
          return Promise.resolve({
            status: "accepted",
            value: { kind: "registering" },
          });
        case "has_input_monitor_failed":
          return Promise.resolve({ status: "accepted", value: false });
        case "is_tray_available":
          return Promise.resolve({ status: "accepted", value: true });
        default:
          throw new Error(`Unexpected command: ${command}`);
      }
    });
  vi.mocked(listen)
    .mockReset()
    .mockImplementation((event, callback) => {
      const callbacks =
        subscriptions.get(event) ?? new Set<EventCallback<unknown>>();
      subscriptions.set(event, callbacks);
      callbacks.add(callback);
      return Promise.resolve(() => {
        callbacks.delete(callback);
      });
    });
});

it("continues observing and notifying during a screen failure without reconnecting on redraw", async () => {
  failure.render = true;
  const view = await act(async () => {
    const rendered = render(<AppRoot />);
    await Promise.resolve();
    return rendered;
  });
  expect(
    screen.getByRole("button", { name: "再表示を試みる" }),
  ).toBeInTheDocument();
  expect(useBackendSyncStore.getState().connection.status).toBe("ready");
  const registrations = vi.mocked(listen).mock.calls.length;
  await act(async () => {
    await Promise.resolve();
    emit("timer_tick", {
      ...createTimerSnapshotForTest().timer,
      remaining_seconds: 123,
    });
    emit("session_completed", {
      transition_reason: "break_waiting_decision",
      completed_session_type: "short_break",
      next_status: "idle",
      next_session_type: "work",
      requires_manual_continue_decision: true,
    } satisfies SessionCompletedPayload);
  });
  expect(useBackendObservationStore.getState().remainingSeconds).toBe(123);
  expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
    title: "作業に戻りましょう！",
    body: "休憩が終わりました。次のポモドーロを始めましょう。",
  });
  failure.render = false;
  fireEvent.click(screen.getByRole("button", { name: "再表示を試みる" }));
  expect(screen.getByText("Remaining: 123")).toBeInTheDocument();
  expect(listen).toHaveBeenCalledTimes(registrations);
  expect(
    [...subscriptions.values()].every((callbacks) => callbacks.size === 1),
  ).toBe(true);
  view.unmount();
  expect(
    [...subscriptions.values()].every((callbacks) => callbacks.size === 0),
  ).toBe(true);
});

it("keeps shortcut guidance current when its warning UI is removed", async () => {
  function Screen({ showNotice }: { showNotice: boolean }): React.ReactElement {
    return (
      <>
        <AppSynchronization />
        <EmergencyShortcutHint />
        {showNotice && <EmergencyShortcutNotice />}
      </>
    );
  }
  const view = await act(async () => {
    const rendered = render(<Screen showNotice />);
    await Promise.resolve();
    return rendered;
  });
  view.rerender(<Screen showNotice={false} />);
  act(() => {
    emit("emergency_shortcut_changed", {
      kind: "registered",
      trigger: "Super + F12",
    });
  });
  expect(screen.getByText("Super + F12")).toBeInTheDocument();
});

it("keeps timer synchronization available when an independent subscription fails", async () => {
  const subscribe = vi.mocked(listen).getMockImplementation();
  if (!subscribe) throw new Error("Missing subscription setup");
  vi.mocked(listen).mockImplementation((event, callback) => {
    if (event === "emergency_shortcut_changed")
      return Promise.reject(new Error("shortcut unavailable"));
    return subscribe(event, callback);
  });
  await act(async () => {
    await Promise.resolve();
    render(<AppRoot />);
  });
  expect(
    screen.getByText("緊急解除のショートカット情報を取得できません。"),
  ).toBeInTheDocument();
  expect(useBackendSyncStore.getState().connection.status).toBe("ready");
  act(() => {
    emit("timer_tick", {
      ...createTimerSnapshotForTest().timer,
      remaining_seconds: 42,
    });
  });
  expect(screen.getByText("Remaining: 42")).toBeInTheDocument();
});

it("releases subscriptions that finish registering after the application unmounts", async () => {
  const finishRegistrations: ((unlisten: UnlistenFn) => void)[] = [];
  vi.mocked(listen).mockImplementation(
    () =>
      new Promise<UnlistenFn>((resolve) => {
        finishRegistrations.push(resolve);
      }),
  );
  const view = render(<AppSynchronization />);
  view.unmount();
  const releases = finishRegistrations.map(() => vi.fn());
  await act(async () => {
    await Promise.resolve();
    finishRegistrations.forEach((finish, index) => {
      const release = releases[index];
      if (!release) throw new Error("Missing release callback");
      finish(release);
    });
  });
  expect(releases.length).toBeGreaterThan(0);
  releases.forEach((release) => {
    expect(release).toHaveBeenCalledExactlyOnceWith();
  });
  expect(invoke).not.toHaveBeenCalledWith("get_full_state");
  expect(invoke).not.toHaveBeenCalledWith("get_emergency_shortcut_status");
  expect(invoke).not.toHaveBeenCalledWith("has_input_monitor_failed");
});
