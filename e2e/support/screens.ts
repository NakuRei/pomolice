import { $, browser, expect } from "@wdio/globals";

export async function requireIdleStart(): Promise<void> {
  let state = "unknown";
  await browser.waitUntil(
    async () => {
      state = await browser.execute(() => {
        const visible = (selector: string): boolean =>
          document.querySelector(selector)?.checkVisibility() ?? false;
        if (visible('[data-testid="continue-dialog-end"]'))
          return "continuation decision";
        if (visible('[data-testid="auto-start-confirm-dialog"]'))
          return "scheduled start confirmation";
        if (visible('[data-testid="emergency-stop-dialog"]'))
          return "emergency stop confirmation";
        if (visible('[data-testid="emergency-stop-button"]')) return "working";
        if (visible('[aria-label="短い休憩"], [aria-label="長い休憩"]'))
          return "break";
        if (visible('[data-testid="start-button"]')) return "idle";
        return "unknown";
      });
      return state !== "unknown";
    },
    {
      timeoutMsg:
        "E2E setup: could not identify the timer screen. Inspect the screenshot and return the E2E app to its idle home screen before retrying.",
    },
  );
  if (state !== "idle") {
    throw new Error(
      `E2E setup: expected idle, found ${state}. Return the E2E app to idle manually (end work or finish the break and choose End; dismiss any confirmation), exit it, then rerun. Test data was not reset.`,
    );
  }
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
