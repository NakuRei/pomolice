/** A session length in seconds as Japanese text, e.g. "25分" or "1分30秒". */
export function formatDurationText(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const remainderSeconds = seconds % 60;
  if (remainderSeconds === 0) {
    return `${String(minutes)}分`;
  }
  if (minutes === 0) {
    return `${String(remainderSeconds)}秒`;
  }
  return `${String(minutes)}分${String(remainderSeconds)}秒`;
}
