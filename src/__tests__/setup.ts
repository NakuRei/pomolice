import "@testing-library/jest-dom";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// Cleanup after each test
afterEach(() => {
  cleanup();
});

// Mock Tauri API
// @ts-expect-error - Mocking Tauri global
global.__TAURI__ = {
  core: {
    invoke: async () => Promise.resolve(),
  },
  event: {
    listen: () => Promise.resolve(() => {}),
    emit: () => Promise.resolve(),
  },
};

// Mock Tauri internals for v2 API
// @ts-expect-error - Mocking Tauri internals
global.__TAURI_INTERNALS__ = {
  invoke: async () => Promise.resolve(),
  convertFileSrc: (filePath: string) => filePath,
};

// Mock window.matchMedia for responsive tests
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {}, // deprecated
    removeListener: () => {}, // deprecated
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => true,
  }),
});
