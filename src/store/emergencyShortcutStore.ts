import type { EmergencyShortcutStatus } from "@/ipc/generated";
import { create } from "zustand";

export const useEmergencyShortcutStore = create<{
  status: EmergencyShortcutStatus | null;
  connectionFailed: boolean;
}>(() => ({ status: null, connectionFailed: false }));
