import { $, browser, expect } from "@wdio/globals";
import { beforeEach, describe, it } from "mocha";
import {
  openTimerSettings,
  requireIdleStart,
  saveTimerSettings,
  waitForBreakDecision,
} from "../support/screens";

describe("Pomodoro user journeys", () => {
  beforeEach(async () => {
    await requireIdleStart();
  });

  it("uses configured durations, completes a break, continues working, then ends the cycle", async () => {
    await $('[data-testid="settings-button"]').click();
    const policy = $('[data-testid="settings-top-post-break-policy-toggle"]');
    await expect(policy).toBeEnabled();
    if ((await policy.getAttribute("aria-pressed")) === "true") {
      await policy.click();
    }
    await expect(policy).toHaveAttribute("aria-pressed", "false");
    await $('[data-testid="settings-top-row-timer"]').click();
    await expect($('[data-testid="settings-timer-page"]')).toBeDisplayed();
    await $('[data-testid="settings-work-duration-input"]').setValue("0.1");
    await $('[data-testid="settings-short-break-input"]').setValue("0.1");
    await $('[data-testid="settings-long-break-input"]').setValue("0.1");
    await saveTimerSettings();
    await expect($('[data-testid="timer-display"]')).toHaveText("00:06");

    await $('[data-testid="start-button"]').click();
    await expect($('[data-testid="emergency-stop-button"]')).toBeDisplayed({
      message: "Starting the configured timer did not begin a work session.",
    });
    await waitForBreakDecision();

    await $('[data-testid="continue-dialog-continue"]').click();
    await expect(
      $('[data-testid="continue-dialog-continue"]'),
    ).not.toBeDisplayed();
    await expect($('[data-testid="emergency-stop-button"]')).toBeDisplayed({
      message: "Choosing Continue did not begin the next work session.",
    });
    await waitForBreakDecision();

    await $('[data-testid="continue-dialog-end"]').click();
    await expect($('[data-testid="start-button"]')).toBeDisplayed({
      message: "Choosing End did not return the timer to idle.",
    });
    await expect($('[data-testid="continue-dialog-end"]')).not.toBeDisplayed();
    await expect($('[data-testid="timer-display"]')).toHaveText("00:06");
  });

  it("interrupts work with emergency stop and can start a fresh work session", async () => {
    await openTimerSettings();
    await $('[data-testid="settings-work-duration-input"]').setValue("10");
    await saveTimerSettings();

    for (let session = 0; session < 2; session += 1) {
      await expect($('[data-testid="timer-display"]')).toHaveText("10:00");
      await $('[data-testid="start-button"]').click();
      await expect($('[data-testid="emergency-stop-button"]')).toBeDisplayed({
        message: `Work session ${session + 1} did not start.`,
      });
      const initialTime = await $('[data-testid="timer-display"]').getText();
      const initialSeconds = initialTime
        .split(":")
        .reduce((total, part) => total * 60 + Number(part), 0);
      await browser.waitUntil(
        async () => {
          const text = await $('[data-testid="timer-display"]').getText();
          const seconds = text
            .split(":")
            .reduce((total, part) => total * 60 + Number(part), 0);
          return seconds > 0 && seconds < initialSeconds;
        },
        {
          timeout: 10_000,
          timeoutMsg: `Work session ${session + 1}: the real timer did not count down.`,
        },
      );

      await $('[data-testid="emergency-stop-button"]').click();
      await expect($('[data-testid="emergency-stop-dialog"]')).toBeDisplayed();
      await $('[data-testid="emergency-stop-confirm"]').click();
      await expect($('[data-testid="start-button"]')).toBeDisplayed({
        message: `Work session ${session + 1}: confirming emergency stop did not return the timer to idle.`,
      });
      await expect(
        $('[data-testid="emergency-stop-dialog"]'),
      ).not.toBeDisplayed();
      await expect($('[data-testid="timer-display"]')).toHaveText("10:00");
    }
  });
});
