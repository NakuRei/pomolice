import { create } from "zustand";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
export type BackendSyncFailure =
  | CommandFailure
  | { status: "setup"; cause: unknown };
export type BackendConnection =
  | { status: "idle" }
  | { status: "connecting"; attempt: number }
  | { status: "retry_wait"; attempt: number; failure: BackendSyncFailure }
  | { status: "ready" }
  | { status: "failed"; failure: BackendSyncFailure };
interface BackendSyncState {
  connection: BackendConnection;
  generation: number;
  setConnection: (connection: BackendConnection) => void;
  reconnect: () => void;
}
export const useBackendSyncStore = create<BackendSyncState>((set) => ({
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
