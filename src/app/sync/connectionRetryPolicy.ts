export const TIMER_SYNC_MAX_CONNECTION_RETRIES = 3;

export function timerSyncRetryDelayMs(attempt: number): number {
  return 1000 * 2 ** attempt;
}

export function shouldRetryTimerSyncConnection(attempt: number): boolean {
  return attempt < TIMER_SYNC_MAX_CONNECTION_RETRIES;
}
