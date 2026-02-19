/**
 * Input Monitor Service
 *
 * NOTE: Input monitoring is now handled entirely by the backend.
 * This file is kept for backward compatibility but the functions are no-ops.
 *
 * The backend:
 * - Monitors global keyboard/mouse input during breaks
 * - Applies penalties directly to timer state
 * - Handles debouncing internally (100ms)
 */

/**
 * Check if we're running in Tauri environment
 */
export function isTauriEnvironment(): boolean {
  // Skip in vitest environment
  if (typeof import.meta !== "undefined" && import.meta.env?.VITEST) {
    return false;
  }

  // In Tauri v2, check if we're in a Tauri context
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// All input monitoring is now backend-managed
// These functions are kept for API compatibility but do nothing

export async function startMonitoring(): Promise<void> {
  // No-op: Backend manages input monitoring
}

export async function stopMonitoring(): Promise<void> {
  // No-op: Backend manages input monitoring
}

export type InputDetectedCallback = () => void;

export async function onInputDetected(
  _callback: InputDetectedCallback
): Promise<(() => void) | null> {
  // No-op: Backend handles input detection and penalties directly
  return null;
}

export async function setupInputListener(
  _callback: InputDetectedCallback
): Promise<() => void> {
  // No-op: Backend handles input detection and penalties directly
  return () => {};
}
