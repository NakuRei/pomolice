import { describe, expect, it } from "vitest";
import {
  WEEKDAY_KEYS,
  convertChronoWeekdayTokenToWeekdayKey,
  convertDateDayIndexToWeekdayKey,
  convertWeekdayKeyToChronoWeekdayToken,
  convertWeekdayKeyToDateDayIndex,
} from "./weeklyAutoRun";

describe("weeklyAutoRun types", () => {
  it("should provide deterministic weekday iteration order", () => {
    expect(WEEKDAY_KEYS).toEqual([
      "mon",
      "tue",
      "wed",
      "thu",
      "fri",
      "sat",
      "sun",
    ]);
  });

  it("should convert Date.getDay() index to weekday key", () => {
    expect(convertDateDayIndexToWeekdayKey(0)).toBe("sun");
    expect(convertDateDayIndexToWeekdayKey(1)).toBe("mon");
    expect(convertDateDayIndexToWeekdayKey(2)).toBe("tue");
    expect(convertDateDayIndexToWeekdayKey(3)).toBe("wed");
    expect(convertDateDayIndexToWeekdayKey(4)).toBe("thu");
    expect(convertDateDayIndexToWeekdayKey(5)).toBe("fri");
    expect(convertDateDayIndexToWeekdayKey(6)).toBe("sat");
  });

  it("should reject invalid Date.getDay() index values", () => {
    expect(() => convertDateDayIndexToWeekdayKey(-1)).toThrow(
      "index must be 0-6",
    );
    expect(() => convertDateDayIndexToWeekdayKey(7)).toThrow(
      "index must be 0-6",
    );
    expect(() => convertDateDayIndexToWeekdayKey(1.1)).toThrow(
      "index must be 0-6",
    );
  });

  it("should convert weekday key to Date.getDay() index", () => {
    expect(convertWeekdayKeyToDateDayIndex("sun")).toBe(0);
    expect(convertWeekdayKeyToDateDayIndex("mon")).toBe(1);
    expect(convertWeekdayKeyToDateDayIndex("tue")).toBe(2);
    expect(convertWeekdayKeyToDateDayIndex("wed")).toBe(3);
    expect(convertWeekdayKeyToDateDayIndex("thu")).toBe(4);
    expect(convertWeekdayKeyToDateDayIndex("fri")).toBe(5);
    expect(convertWeekdayKeyToDateDayIndex("sat")).toBe(6);
  });

  it("should convert chrono weekday tokens to weekday keys", () => {
    expect(convertChronoWeekdayTokenToWeekdayKey("Mon")).toBe("mon");
    expect(convertChronoWeekdayTokenToWeekdayKey("Tue")).toBe("tue");
    expect(convertChronoWeekdayTokenToWeekdayKey("Wed")).toBe("wed");
    expect(convertChronoWeekdayTokenToWeekdayKey("Thu")).toBe("thu");
    expect(convertChronoWeekdayTokenToWeekdayKey("Fri")).toBe("fri");
    expect(convertChronoWeekdayTokenToWeekdayKey("Sat")).toBe("sat");
    expect(convertChronoWeekdayTokenToWeekdayKey("Sun")).toBe("sun");
    expect(convertChronoWeekdayTokenToWeekdayKey("mon")).toBe("mon");
  });

  it("should convert weekday keys to chrono weekday tokens", () => {
    expect(convertWeekdayKeyToChronoWeekdayToken("mon")).toBe("Mon");
    expect(convertWeekdayKeyToChronoWeekdayToken("tue")).toBe("Tue");
    expect(convertWeekdayKeyToChronoWeekdayToken("wed")).toBe("Wed");
    expect(convertWeekdayKeyToChronoWeekdayToken("thu")).toBe("Thu");
    expect(convertWeekdayKeyToChronoWeekdayToken("fri")).toBe("Fri");
    expect(convertWeekdayKeyToChronoWeekdayToken("sat")).toBe("Sat");
    expect(convertWeekdayKeyToChronoWeekdayToken("sun")).toBe("Sun");
  });
});
