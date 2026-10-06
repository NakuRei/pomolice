import { spawn } from "node:child_process";
import { once } from "node:events";
import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import { appBinaryPath } from "../support/app-binary";

type TauriWindowGlobal = {
  __TAURI__: {
    window: {
      getCurrentWindow(): {
        hide(): Promise<void>;
        isVisible(): Promise<boolean>;
      };
    };
  };
};

async function isMainWindowVisible(): Promise<boolean> {
  return browser.execute(() =>
    (window as unknown as TauriWindowGlobal).__TAURI__.window
      .getCurrentWindow()
      .isVisible(),
  );
}

describe("Single instance", () => {
  it("brings the hidden window back when the app is launched again", async () => {
    await expect($('[data-testid="start-button"]')).toBeDisplayed();
    await browser.execute(() =>
      (window as unknown as TauriWindowGlobal).__TAURI__.window
        .getCurrentWindow()
        .hide(),
    );
    expect(await isMainWindowVisible()).toBe(false);

    const secondLaunch = spawn(appBinaryPath, { stdio: "ignore" });
    const exitTimeout = setTimeout(() => secondLaunch.kill(), 15_000);
    const [exitCode] = (await once(secondLaunch, "exit")) as [number | null];
    clearTimeout(exitTimeout);

    if (exitCode !== 0) {
      throw new Error(
        `The second launch kept running or failed (exit code ${exitCode}) instead of handing off to the running app.`,
      );
    }
    await browser.waitUntil(isMainWindowVisible, {
      timeout: 10_000,
      timeoutMsg: "Launching the app again did not reveal the hidden window.",
    });
    await expect($('[data-testid="start-button"]')).toBeDisplayed();
  });
});
