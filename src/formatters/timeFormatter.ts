/**
 * Time formatting utilities
 * Converts seconds to human-readable time formats
 */

/**
 * Converts seconds to MM:SS format
 * @param seconds - Total seconds to convert
 * @returns Formatted time string in MM:SS format
 *
 * @example
 * formatTime(0) // "00:00"
 * formatTime(65) // "01:05"
 * formatTime(1500) // "25:00"
 * formatTime(3661) // "61:01"
 */
export function formatTime(seconds: number): string {
  // Ensure non-negative value
  const totalSeconds = Math.max(0, Math.floor(seconds));

  const minutes = Math.floor(totalSeconds / 60);
  const remainingSeconds = totalSeconds % 60;

  // Pad with zeros to ensure 2 digits
  const minutesStr = String(minutes).padStart(2, "0");
  const secondsStr = String(remainingSeconds).padStart(2, "0");

  return `${minutesStr}:${secondsStr}`;
}
