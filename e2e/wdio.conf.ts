import { access, mkdir } from "node:fs/promises";
import path from "node:path";
import { browser } from "@wdio/globals";
import "@wdio/tauri-service";

const root = path.resolve(import.meta.dirname, "..");
let results = path.join(
  root,
  "artifacts",
  "e2e",
  `${new Date().toISOString().replaceAll(":", "-")}-${process.pid}`,
);
const appBinaryPath = path.join(root, "target", "e2e", "debug", "pomolice.exe");

async function saveFailureScreenshot(title: string): Promise<void> {
  try {
    if (!browser.sessionId) return;
    const name = title.replaceAll(/[^a-zA-Z0-9_-]/g, "-").slice(0, 140);
    const screenshot = path.join(results, `${name}-${Date.now()}.png`);
    await browser.saveScreenshot(screenshot);
    console.info(`Failure screenshot: ${screenshot}`);
  } catch (error) {
    console.warn("Could not capture the failed test's window:", error);
  }
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: [path.join(import.meta.dirname, "specs", "*.spec.ts")],
  maxInstances: 1,
  injectGlobals: false,
  logLevel: "info",
  outputDir: results,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { timeout: 90_000, bail: true },
  waitforTimeout: 15_000,
  connectionRetryCount: 0,
  connectionRetryTimeout: 15_000,
  services: [
    [
      "tauri",
      {
        appBinaryPath,
        driverProvider: "external",
        autoInstallTauriDriver: true,
        autoDownloadEdgeDriver: true,
        captureBackendLogs: true,
        backendLogLevel: "info",
      },
    ],
  ],
  capabilities: [{ browserName: "tauri" }],
  onPrepare: async (launcherConfig) => {
    if (launcherConfig.outputDir === undefined) {
      throw new Error("E2E requires an outputDir for logs and screenshots.");
    }
    results = launcherConfig.outputDir;
    console.info(`E2E results: ${results}`);
    if (process.platform !== "win32") {
      throw new Error(
        "These E2E tests require an interactive Windows desktop.",
      );
    }
    await access(appBinaryPath);
    await mkdir(results, { recursive: true });
  },
  onWorkerStart: (_cid, _capabilities, _specs, args) => {
    args.outputDir = results;
  },
  beforeSession: (workerConfig) => {
    if (workerConfig.outputDir === undefined) {
      throw new Error("E2E requires an outputDir for logs and screenshots.");
    }
    results = workerConfig.outputDir;
  },
  afterTest: async (test, _context, { passed }) => {
    if (!passed) {
      await saveFailureScreenshot(test.title);
    }
  },
  afterHook: async (test, _context, { error }, hookName) => {
    if (error !== undefined && error !== null) {
      await saveFailureScreenshot(`${test.title}-${hookName}`);
    }
  },
  onComplete: (exitCode, launcherConfig) => {
    console.info(
      `E2E finished (exit ${exitCode}). Results: ${launcherConfig.outputDir}`,
    );
  },
};
