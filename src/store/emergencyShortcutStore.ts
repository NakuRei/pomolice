import { create } from "zustand";
import type { EmergencyShortcutStatus } from "@/types/emergencyShortcut";

export const useEmergencyShortcutStore = create<{
  status: EmergencyShortcutStatus | null;
  connectionFailed: boolean;
}>(() => ({ status: null, connectionFailed: false }));
