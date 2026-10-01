import { create } from "zustand";

interface TrayState {
  status: "checking" | "available" | "unavailable" | "unknown";
  checkAttempt: number;
  exitStatus: "idle" | "exiting" | "failed";
  retry: () => void;
}

export const useTrayStore = create<TrayState>((set) => ({
  status: "checking",
  checkAttempt: 0,
  exitStatus: "idle",
  retry: (): void => {
    set((state) => ({
      status: "checking",
      checkAttempt: state.checkAttempt + 1,
    }));
  },
}));
