/**
 * Application screen identifier used by App-level view switching.
 *
 * Usage example:
 * ```ts
 * const initialScreen: AppScreen = "home";
 * ```
 */
export type AppScreen =
  | "home"
  | "settingsTop"
  | "settingsTimer"
  | "settingsAutoRun"
  | "settingsNotification";

export const SETTINGS_SCREEN_VALUES = [
  "settingsTop",
  "settingsTimer",
  "settingsAutoRun",
  "settingsNotification",
] as const satisfies readonly AppScreen[];

export type SettingsScreen = (typeof SETTINGS_SCREEN_VALUES)[number];
