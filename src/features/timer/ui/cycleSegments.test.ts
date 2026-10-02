import { describe, it, expect } from "vitest";
import {
  deriveCycleSegments,
  type CycleSegment,
  type CycleSegmentsInput,
} from "./cycleSegments";

const BASE_INPUT: CycleSegmentsInput = {
  currentCycle: 0,
  sessionType: "work",
  remainingSeconds: 1500,
  sessionDurationSeconds: 1500,
  longBreakInterval: 4,
};

function sessionSegment(
  input: CycleSegmentsInput,
  kind: CycleSegment["kind"],
  cycleNumber: number,
): CycleSegment {
  const segment = deriveCycleSegments(input).find(
    (each) => each.kind === kind && each.cycleNumber === cycleNumber,
  );
  if (!segment) {
    throw new Error(`Missing ${kind} session in cycle ${String(cycleNumber)}`);
  }

  return segment;
}

describe("deriveCycleSegments", () => {
  it("pairs every cycle with a break and ends on the long break", () => {
    const segments = deriveCycleSegments(BASE_INPUT);

    expect(
      segments.map(({ kind, cycleNumber }) => ({ kind, cycleNumber })),
    ).toEqual([
      { kind: "work", cycleNumber: 1 },
      { kind: "break", cycleNumber: 1 },
      { kind: "work", cycleNumber: 2 },
      { kind: "break", cycleNumber: 2 },
      { kind: "work", cycleNumber: 3 },
      { kind: "break", cycleNumber: 3 },
      { kind: "work", cycleNumber: 4 },
      { kind: "longBreak", cycleNumber: 4 },
    ]);
  });

  it("gives the long break more room than a short break, and work the most", () => {
    const segments = deriveCycleSegments(BASE_INPUT);

    const workFlexGrow = sessionSegment(BASE_INPUT, "work", 1).flexGrow;
    const shortBreakFlexGrow = sessionSegment(BASE_INPUT, "break", 1).flexGrow;
    const longBreakFlexGrow = sessionSegment(
      BASE_INPUT,
      "longBreak",
      4,
    ).flexGrow;

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
    const input: CycleSegmentsInput = {
      ...BASE_INPUT,
      currentCycle: 1,
      sessionType: "work",
      remainingSeconds: 1125,
    };

    const workSegment = sessionSegment(input, "work", 1);
    expect(workSegment.state).toBe("active");
    expect(workSegment.fillRatio).toBeCloseTo(0.25);

    expect(sessionSegment(input, "break", 1).state).toBe("upcoming");
    expect(sessionSegment(input, "work", 2).state).toBe("upcoming");
  });

  it("completes the work segment once its break has started", () => {
    const input: CycleSegmentsInput = {
      ...BASE_INPUT,
      currentCycle: 1,
      sessionType: "short_break",
      remainingSeconds: 150,
      sessionDurationSeconds: 300,
    };

    const workSegment = sessionSegment(input, "work", 1);
    expect(workSegment.state).toBe("completed");
    expect(workSegment.fillRatio).toBe(1);

    const breakSegment = sessionSegment(input, "break", 1);
    expect(breakSegment.state).toBe("active");
    expect(breakSegment.fillRatio).toBeCloseTo(0.5);
  });

  it("completes every segment of the cycles already passed", () => {
    const input: CycleSegmentsInput = {
      ...BASE_INPUT,
      currentCycle: 3,
      sessionType: "work",
      remainingSeconds: 1500,
    };

    for (const segment of deriveCycleSegments(input).slice(0, 4)) {
      expect(segment.state).toBe("completed");
      expect(segment.fillRatio).toBe(1);
    }

    expect(sessionSegment(input, "work", 3).state).toBe("active");
    expect(sessionSegment(input, "break", 3).state).toBe("upcoming");
  });

  it("measures the running session against the duration it started with", () => {
    const input: CycleSegmentsInput = {
      ...BASE_INPUT,
      currentCycle: 4,
      sessionType: "long_break",
      remainingSeconds: 300,
      sessionDurationSeconds: 1200,
    };

    const longBreakSegment = sessionSegment(input, "longBreak", 4);
    expect(longBreakSegment.state).toBe("active");
    expect(longBreakSegment.fillRatio).toBeCloseTo(0.75);
  });

  it("shows a cycle past the interval as the last cycle, before the long break", () => {
    const input: CycleSegmentsInput = {
      ...BASE_INPUT,
      currentCycle: 5,
      sessionType: "work",
      remainingSeconds: 750,
      longBreakInterval: 3,
    };

    for (const segment of deriveCycleSegments(input).slice(0, 4)) {
      expect(segment.state).toBe("completed");
    }
    const lastWork = sessionSegment(input, "work", 3);
    expect(lastWork.state).toBe("active");
    expect(lastWork.fillRatio).toBeCloseTo(0.5);
    expect(sessionSegment(input, "longBreak", 3).state).toBe("upcoming");
  });

  it("follows the configured cycle count", () => {
    const input: CycleSegmentsInput = { ...BASE_INPUT, longBreakInterval: 2 };

    expect(
      deriveCycleSegments(input).map(({ kind, cycleNumber }) => ({
        kind,
        cycleNumber,
      })),
    ).toEqual([
      { kind: "work", cycleNumber: 1 },
      { kind: "break", cycleNumber: 1 },
      { kind: "work", cycleNumber: 2 },
      { kind: "longBreak", cycleNumber: 2 },
    ]);
  });

  it("keeps the fill within bounds for out-of-range remaining time", () => {
    const overrun = sessionSegment(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: -60,
      },
      "work",
      1,
    );
    expect(overrun.fillRatio).toBe(1);

    const beforeStart = sessionSegment(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: 9999,
      },
      "work",
      1,
    );
    expect(beforeStart.fillRatio).toBe(0);
  });

  it("stays empty when the session duration is zero", () => {
    const segment = sessionSegment(
      {
        ...BASE_INPUT,
        currentCycle: 1,
        remainingSeconds: 0,
        sessionDurationSeconds: 0,
      },
      "work",
      1,
    );

    expect(segment.fillRatio).toBe(0);
  });
});
