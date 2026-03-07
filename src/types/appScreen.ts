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
