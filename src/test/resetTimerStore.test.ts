import { beforeEach, describe, expect, it } from "vitest";
import { TimerStatus, useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "./resetTimerStore";

type TimerStoreSnapshot = ReturnType<typeof useTimerStore.getState>;

function readTimerStoreDataSnapshot(
  state: TimerStoreSnapshot,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(state).filter(([, value]) => typeof value !== "function"),
  );
}

function readTimerStoreActionNames(
  state: TimerStoreSnapshot,
): (keyof TimerStoreSnapshot)[] {
  const actionNames: (keyof TimerStoreSnapshot)[] = [];

  for (const name of Object.keys(state) as (keyof TimerStoreSnapshot)[]) {
    if (typeof state[name] === "function") {
      actionNames.push(name);
    }
  }

  return actionNames;
}

describe("resetTimerStoreForTest", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
  });

  it("should restore timer store data to the store initial data", () => {
    const initialDataSnapshot = readTimerStoreDataSnapshot(
      useTimerStore.getInitialState(),
    );

    useTimerStore.setState({
      status: TimerStatus.ON_BREAK,
      remainingSeconds: 42,
      currentCycle: 3,
      isConnected: true,
      infoMessage: {
        id: "auto_run_stopped",
        message: "custom",
        ttlMs: 1,
      },
    });

    resetTimerStoreForTest();

    expect(readTimerStoreDataSnapshot(useTimerStore.getState())).toEqual(
      initialDataSnapshot,
    );
  });

  it("should keep timer store actions callable after reset", () => {
    const initialActionNames = readTimerStoreActionNames(
      useTimerStore.getInitialState(),
    );
    useTimerStore.setState({
      status: TimerStatus.WORKING,
      currentCycle: 2,
    });

    resetTimerStoreForTest();

    const resetState = useTimerStore.getState();
    expect(initialActionNames).not.toHaveLength(0);
    for (const actionName of initialActionNames) {
      expect(resetState[actionName]).toBeTypeOf("function");
    }
  });
});
