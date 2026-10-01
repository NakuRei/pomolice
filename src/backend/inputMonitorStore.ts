import { create } from "zustand";

interface InputMonitorState {
  failureObserved: boolean;
  connectionFailed: boolean;
  attempt: number;
  retry: () => void;
}

export const useInputMonitorStore = create<InputMonitorState>((set) => ({
  failureObserved: false,
  connectionFailed: false,
  attempt: 0,
  retry: (): void => {
    set((state) => ({ attempt: state.attempt + 1 }));
  },
}));
