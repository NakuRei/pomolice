import "@testing-library/jest-dom";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

function doNothing(): void {
  return undefined;
}

function noopCleanup(): void {
  return undefined;
}

afterEach(cleanup);

// Mock Tauri API
Object.assign(globalThis, {
  __TAURI__: {
    core: {
      invoke: async (): Promise<void> => Promise.resolve(),
    },
    event: {
      listen: (): Promise<() => void> => Promise.resolve(noopCleanup),
      emit: (): Promise<void> => Promise.resolve(),
    },
  },
});

// Mock Tauri internals for v2 API
Object.assign(globalThis, {
  __TAURI_INTERNALS__: {
    invoke: async (): Promise<void> => Promise.resolve(),
    convertFileSrc: (filePath: string): string => filePath,
  },
});

// Mock window.matchMedia for responsive tests
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string): MediaQueryList => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: doNothing,
    removeListener: doNothing,
    addEventListener: doNothing,
    removeEventListener: doNothing,
    dispatchEvent: (): boolean => true,
  }),
});
