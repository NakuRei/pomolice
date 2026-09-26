import { describe, it, expect } from "vitest";
import { formatDurationText } from "./durationText";

describe("formatDurationText", () => {
  it.each([
    { seconds: 25 * 60, expected: "25分" },
    { seconds: 45, expected: "45秒" },
    { seconds: 90, expected: "1分30秒" },
    { seconds: 1, expected: "1秒" },
  ])("formats $seconds seconds as $expected", ({ seconds, expected }) => {
    expect(formatDurationText(seconds)).toBe(expected);
  });
});
