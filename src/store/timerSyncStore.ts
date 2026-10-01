import { create } from "zustand";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
export type TimerSyncFailure =
  | CommandFailure
  | { status: "setup"; cause: unknown };
export type TimerConnection =
  | { status: "idle" }
  | { status: "connecting"; attempt: number }
  | { status: "retry_wait"; attempt: number; failure: TimerSyncFailure }
  | { status: "ready" }
  | { status: "failed"; failure: TimerSyncFailure };
interface TimerSyncState {
  connection: TimerConnection;
  generation: number;
  setConnection: (connection: TimerConnection) => void;
  reconnect: () => void;
}
export const useTimerSyncStore = create<TimerSyncState>((set) => ({
  connection: { status: "idle" },
  generation: 0,
  setConnection: (connection): void => {
    set({ connection });
  },
  reconnect: (): void => {
    set((state) => ({
      generation: state.generation + 1,
      connection: { status: "connecting", attempt: 0 },
    }));
  },
}));
