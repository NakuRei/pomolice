import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";
import {
  openTimerSettings,
  saveTimerSettings,
  startWork,
  waitForBreakDecision,
} from "../support/screens";

describe("Pomodoro user journeys", () => {
  it("uses configured durations, completes a break, continues working, then ends the cycle", async () => {
    await openTimerSettings();
    await $('[data-testid="settings-work-duration-input"]').setValue("0.1");
    await $('[data-testid="settings-short-break-input"]').setValue("0.1");
    await $('[data-testid="settings-long-break-input"]').setValue("0.1");
    await saveTimerSettings();
    await expect($('[data-testid="timer-display"]')).toHaveText("00:06");

    await startWork(
      "Starting the configured timer did not begin a work session.",
    );
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
      await startWork(`Work session ${session + 1} did not start.`);
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

  it("plays every bundled notification sound from the settings preview", async () => {
    await $('[data-testid="settings-button"]').click();
    for (const soundType of ["work", "short_break", "long_break"]) {
      // Reopening the page clears the previous preview's status message.
      await $('[data-testid="settings-top-row-notification"]').click();
      await $(
        '[data-testid="settings-notification-sound-preview-type"]',
      ).selectByAttribute("value", soundType);
      await $(
        '[data-testid="settings-notification-sound-preview-button"]',
      ).click();
      await expect(
        $('[data-testid="settings-notification-sound-preview-status"]'),
      ).toHaveText("通知音を再生しました。", {
        message: `The ${soundType} notification sound preview did not play.`,
      });
      await $('[data-testid="settings-cancel"]').click();
      await expect($('[data-testid="settings-top-page"]')).toBeDisplayed();
    }
    await $('[data-testid="settings-top-back-home"]').click();
    await expect($('[data-testid="start-button"]')).toBeDisplayed();
  });
});
