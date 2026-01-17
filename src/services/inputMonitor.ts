/**
 * Input Monitor Service
 * Wrapper for Tauri input monitoring commands and event listeners
 * Phase 4: Break time penalty system
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Input event types detected during break
 */
export type InputEventType = 'Keyboard' | 'MouseClick' | 'MouseMove';

/**
 * Payload received from backend when input is detected
 */
export interface InputDetectedPayload {
  event_type: { type: InputEventType };
  timestamp: number;
}

/**
 * Callback function type for input detection
 */
export type InputDetectedCallback = (payload: InputDetectedPayload) => void;

/**
 * Check if we're running in Tauri environment
 */
function isTauriEnvironment(): boolean {
  // Skip in vitest environment
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITEST) {
    return false;
  }

  // In Tauri v2, check if we're in a Tauri context
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Start global input monitoring
 * This will begin listening to all keyboard and mouse events system-wide
 */
export async function startMonitoring(): Promise<void> {
  throw new Error('startMonitoring is backend-managed. Do not call from renderer.');
}

/**
 * Stop global input monitoring
 */
export async function stopMonitoring(): Promise<void> {
  throw new Error('stopMonitoring is backend-managed. Do not call from renderer.');
}

/**
 * Listen for input-detected events
 * Returns an unlisten function to stop listening
 */
export async function onInputDetected(
  callback: InputDetectedCallback
): Promise<UnlistenFn | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  try {
    const unlisten = await listen<InputDetectedPayload>('input-detected', (event) => {
      callback(event.payload);
    });

    return unlisten;
  } catch (error) {
    console.error('Failed to register input detection listener:', error);
    throw error;
  }
}

/**
 * Helper to create a managed input listener that automatically cleans up
 * Usage:
 *   const cleanup = await setupInputListener((payload) => { ... });
 *   // Later:
 *   cleanup();
 */
export async function setupInputListener(
  callback: InputDetectedCallback
): Promise<() => void> {
  const unlisten = await onInputDetected(callback);

  return () => {
    if (unlisten) {
      unlisten();
    }
  };
}
