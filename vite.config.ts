/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const tauriDevHost = process.env.TAURI_DEV_HOST;
const hasTauriDevHost =
  tauriDevHost !== undefined && tauriDevHost.trim() !== "";
const serverHost = hasTauriDevHost ? tauriDevHost : false;
const hmrConfig = hasTauriDevHost
  ? {
      protocol: "ws",
      host: tauriDevHost,
      port: 1421,
    }
  : undefined;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: serverHost,
    hmr: hmrConfig,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // Vitest configuration
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/__tests__/setup.ts",
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "src/__tests__/",
        "**/*.test.{ts,tsx}",
        "**/*.config.{ts,js}",
      ],
    },
  },
}));
