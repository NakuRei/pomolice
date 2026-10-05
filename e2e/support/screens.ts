import { $, browser, expect } from "@wdio/globals";

// The timer rejects a manual start made right after the previous transition,
// and WebDriver clicks faster than a person can, so a rejected click is retried.
export async function startWork(failureMessage: string): Promise<void> {
  const startButton = $('[data-testid="start-button"]');
  const emergencyStopButton = $('[data-testid="emergency-stop-button"]');
  await browser.waitUntil(
    async () => {
      if (await startButton.isDisplayed()) await startButton.click();
      return emergencyStopButton.isDisplayed();
    },
    { interval: 500, timeoutMsg: failureMessage },
  );
}

export async function openTimerSettings(): Promise<void> {
  await $('[data-testid="settings-button"]').click();
  await $('[data-testid="settings-top-row-timer"]').click();
  await expect($('[data-testid="settings-timer-page"]')).toBeDisplayed();
}

export async function saveTimerSettings(): Promise<void> {
  await $('[data-testid="settings-save"]').click();
  await expect($('[data-testid="settings-top-page"]')).toBeDisplayed();
  await $('[data-testid="settings-top-back-home"]').click();
  await expect($('[data-testid="start-button"]')).toBeDisplayed();
}

export async function waitForBreakDecision(): Promise<void> {
  await expect(
    $('[aria-label="短い休憩"], [aria-label="長い休憩"]'),
  ).toBeDisplayed({
    wait: 20_000,
    message: "The work session did not transition to a break.",
  });
  await expect($('[data-testid="continue-dialog-end"]')).toBeDisplayed({
    wait: 20_000,
    message: "The break did not finish with a continuation decision.",
  });
}
