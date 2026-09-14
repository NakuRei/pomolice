export type EmergencyShortcutStatus =
  | { kind: "input_monitor" }
  | { kind: "registering" }
  | { kind: "registered"; trigger: string }
  | { kind: "unavailable" };
