import { describe, it, expect } from "vitest";
import { deriveCycleSegments, type CycleSegmentsInput } from "./cycleSegments";
import { SessionType } from "@/store/timerStore";

const BASE_INPUT: CycleSegmentsInput = {
  currentCycle: 0,
  sessionType: SessionType.WORK,
  remainingSeconds: 1500,
  longBreakInterval: 4,
  workDurationSeconds: 1500,
  shortBreakDurationSeconds: 300,
  longBreakDurationSeconds: 1200,
};

function segmentByKey(
  input: CycleSegmentsInput,
  key: string,
): ReturnType<typeof deriveCycleSegments>[number] {
  const segment = deriveCycleSegments(input).find((each) => each.key === key);
  if (!segment) {
    throw new Error(`segment ${key} not found`);
  }

  return segment;
}

describe("deriveCycleSegments", () => {
  it("pairs every cycle with a break and ends on the long break", () => {
    const segments = deriveCycleSegments(BASE_INPUT);

    expect(segments.map((segment) => segment.key)).toEqual([
      "work-1",
      "break-1",
      "work-2",
      "break-2",
      "work-3",
      "break-3",
      "work-4",
      "break-4",
    ]);
    expect(segmentByKey(BASE_INPUT, "break-4").kind).toBe("longBreak");
  });

  it("gives the long break more room than a short break, and work the most", () => {
    const segments = deriveCycleSegments(BASE_INPUT);

    const workFlexGrow = segmentByKey(BASE_INPUT, "work-1").flexGrow;
    const shortBreakFlexGrow = segmentByKey(BASE_INPUT, "break-1").flexGrow;
    const longBreakFlexGrow = segmentByKey(BASE_INPUT, "break-4").flexGrow;

    expect(shortBreakFlexGrow).toBeLessThan(longBreakFlexGrow);
    expect(longBreakFlexGrow).toBeLessThan(workFlexGrow);
    expect(segments).toHaveLength(8);
  });

  it("leaves every segment empty before the first start", () => {
    const segments = deriveCycleSegments(BASE_INPUT);

    expect(
      segments.every(
        (segment) => segment.state === "upcoming" && segment.fillRatio === 0,
      ),
    ).toBe(true);
  });

  it("fills the running work segment by elapsed time", () => {
    const input = {
      ...BASE_INPUT,
      currentCycle: 1,
      sessionType: SessionType.WORK,
      remainingSeconds: 1125,
    };

    const workSegment = segmentByKey(input, "work-1");
    expect(workSegment.state).toBe("active");
    expect(workSegment.fillRatio).toBeCloseTo(0.25);

    expect(segmentByKey(input, "break-1").state).toBe("upcoming");
    expect(segmentByKey(input, "work-2").state).toBe("upcoming");
  });

  it("completes the work segment once its break has started", () => {
    const input = {
      ...BASE_INPUT,
      currentCycle: 1,
      sessionType: SessionType.SHORT_BREAK,
      remainingSeconds: 150,
    };

    const workSegment = segmentByKey(input, "work-1");
    expect(workSegment.state).toBe("completed");
    expect(workSegment.fillRatio).toBe(1);

    const breakSegment = segmentByKey(input, "break-1");
    expect(breakSegment.state).toBe("active");
    expect(breakSegment.fillRatio).toBeCloseTo(0.5);
  });

  it("completes every segment of the cycles already passed", () => {
    const input = {
      ...BASE_INPUT,
      currentCycle: 3,
      sessionType: SessionType.WORK,
      remainingSeconds: 1500,
    };

    for (const key of ["work-1", "break-1", "work-2", "break-2"]) {
      const segment = segmentByKey(input, key);
      expect(segment.state).toBe("completed");
      expect(segment.fillRatio).toBe(1);
    }

    expect(segmentByKey(input, "work-3").state).toBe("active");
    expect(segmentByKey(input, "break-3").state).toBe("upcoming");
  });

  it("measures the long break against the long break duration", () => {
    const input = {
      ...BASE_INPUT,
      currentCycle: 4,
      sessionType: SessionType.LONG_BREAK,
      remainingSeconds: 300,
    };

    const longBreakSegment = segmentByKey(input, "break-4");
    expect(longBreakSegment.state).toBe("active");
    expect(longBreakSegment.fillRatio).toBeCloseTo(0.75);
  });

  it("follows the configured cycle count", () => {
    const input = { ...BASE_INPUT, longBreakInterval: 2 };

    expect(deriveCycleSegments(input).map((segment) => segment.key)).toEqual([
      "work-1",
      "break-1",
      "work-2",
      "break-2",
    ]);
    expect(segmentByKey(input, "break-2").kind).toBe("longBreak");
  });

  it("keeps the fill within bounds for out-of-range remaining time", () => {
    const overrun = segmentByKey(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: -60,
      },
      "work-1",
    );
    expect(overrun.fillRatio).toBe(1);

    const beforeStart = segmentByKey(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: 9999,
      },
      "work-1",
    );
    expect(beforeStart.fillRatio).toBe(0);
  });

  it("stays empty when the configured duration is zero", () => {
    const segment = segmentByKey(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: 0,
        workDurationSeconds: 0,
      },
      "work-1",
    );

    expect(segment.fillRatio).toBe(0);
  });
});
