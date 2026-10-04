import { spawnSync } from "node:child_process";
import path from "node:path";

if (process.platform !== "win32") {
  throw new Error("The E2E build currently targets Windows.");
}
const root = path.resolve(import.meta.dirname, "..");
const result = spawnSync(
  process.execPath,
  [
    "node_modules/@tauri-apps/cli/tauri.js",
    "build",
    "--debug",
    "--no-bundle",
    "--features",
    "e2e",
    "--config",
    "src-tauri/tauri.e2e.conf.json",
  ],
  {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
    env: { ...process.env, CARGO_TARGET_DIR: path.join(root, "target", "e2e") },
  },
);
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
