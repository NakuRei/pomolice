import { attachConsole } from "@tauri-apps/plugin-log";

let frontendLogDevtoolsInitializationPromise: Promise<void> | null = null;
let developmentEnvironmentOverride: boolean | undefined = undefined;

function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function isDevelopmentEnvironment(): boolean {
  return developmentEnvironmentOverride ?? import.meta.env.DEV;
}

export async function initializeFrontendLogDevtools(): Promise<void> {
  if (!isDevelopmentEnvironment() || !isTauriEnvironment()) {
    return;
  }

  if (frontendLogDevtoolsInitializationPromise) {
    return frontendLogDevtoolsInitializationPromise;
  }

  frontendLogDevtoolsInitializationPromise = attachConsole()
    .then(() => undefined)
    .catch(() => {
      frontendLogDevtoolsInitializationPromise = null;
    });

  return frontendLogDevtoolsInitializationPromise;
}

export function setDevelopmentEnvironmentOverrideForTest(
  isDevelopment: boolean | undefined,
): void {
  developmentEnvironmentOverride = isDevelopment;
}
