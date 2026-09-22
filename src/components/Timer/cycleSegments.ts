import { SessionType, type SessionTypeValue } from "@/store/timerStore";

/**
 * Visual state of one segment in the cycle bar.
 *
 * Usage example:
 * ```ts
 * const state: CycleSegmentState = "active";
 * ```
 */
export type CycleSegmentState = "completed" | "active" | "upcoming";

/**
 * One segment of the cycle bar: either a work session or the break that follows it.
 *
 * Usage example:
 * ```ts
 * const segment: CycleSegment = {
 *   key: "work-1",
 *   kind: "work",
 *   cycleNumber: 1,
 *   flexGrow: 5,
 *   fillRatio: 0.4,
 *   state: "active",
 * };
 * ```
 */
export interface CycleSegment {
  key: string;
  kind: "work" | "break" | "longBreak";
  cycleNumber: number;
  flexGrow: number;
  fillRatio: number;
  state: CycleSegmentState;
}

/**
 * Display state needed to lay out the cycle bar.
 *
 * Durations are the configured session lengths in seconds; they set the
 * denominator for the in-progress segment's fill.
 *
 * Usage example:
 * ```ts
 * const input: CycleSegmentsInput = {
 *   currentCycle: 2,
 *   sessionType: "work",
 *   remainingSeconds: 900,
 *   longBreakInterval: 4,
 *   workDurationSeconds: 1500,
 *   shortBreakDurationSeconds: 300,
 *   longBreakDurationSeconds: 1200,
 * };
 * ```
 */
export interface CycleSegmentsInput {
  currentCycle: number;
  sessionType: SessionTypeValue;
  remainingSeconds: number;
  longBreakInterval: number;
  workDurationSeconds: number;
  shortBreakDurationSeconds: number;
  longBreakDurationSeconds: number;
}

const WORK_SEGMENT_FLEX_GROW = 5;
const SHORT_BREAK_SEGMENT_FLEX_GROW = 1;
const LONG_BREAK_SEGMENT_FLEX_GROW = 2;

interface SegmentProgress {
  fillRatio: number;
  state: CycleSegmentState;
}

function activeSessionDurationSeconds(input: CycleSegmentsInput): number {
  const durationSecondsBySessionType: Record<SessionTypeValue, number> = {
    [SessionType.WORK]: input.workDurationSeconds,
    [SessionType.SHORT_BREAK]: input.shortBreakDurationSeconds,
    [SessionType.LONG_BREAK]: input.longBreakDurationSeconds,
  };

  return durationSecondsBySessionType[input.sessionType];
}

function activeSessionElapsedRatio(input: CycleSegmentsInput): number {
  const totalSeconds = activeSessionDurationSeconds(input);
  if (totalSeconds <= 0) {
    return 0;
  }

  const elapsedRatio = 1 - input.remainingSeconds / totalSeconds;
  return Math.min(1, Math.max(0, elapsedRatio));
}

function resolveSegmentProgress(
  isFinished: boolean,
  isActive: boolean,
  elapsedRatio: number,
): SegmentProgress {
  if (isFinished) {
    return { fillRatio: 1, state: "completed" };
  }
  if (isActive) {
    return { fillRatio: elapsedRatio, state: "active" };
  }

  return { fillRatio: 0, state: "upcoming" };
}

function buildWorkSegment(
  cycleNumber: number,
  progress: SegmentProgress,
): CycleSegment {
  return {
    key: `work-${String(cycleNumber)}`,
    kind: "work",
    cycleNumber,
    flexGrow: WORK_SEGMENT_FLEX_GROW,
    ...progress,
  };
}

function buildBreakSegment(
  cycleNumber: number,
  isLongBreak: boolean,
  progress: SegmentProgress,
): CycleSegment {
  return {
    key: `break-${String(cycleNumber)}`,
    kind: isLongBreak ? "longBreak" : "break",
    cycleNumber,
    flexGrow: isLongBreak
      ? LONG_BREAK_SEGMENT_FLEX_GROW
      : SHORT_BREAK_SEGMENT_FLEX_GROW,
    ...progress,
  };
}

/**
 * Builds the cycle bar segments for the current timer state.
 *
 * Each cycle contributes a work segment followed by its break; the break of the
 * last cycle is the long break. Before the first start (`currentCycle` is 0)
 * every segment is upcoming and empty.
 *
 * Usage example:
 * ```ts
 * const segments = deriveCycleSegments({
 *   currentCycle: 1,
 *   sessionType: "work",
 *   remainingSeconds: 750,
 *   longBreakInterval: 4,
 *   workDurationSeconds: 1500,
 *   shortBreakDurationSeconds: 300,
 *   longBreakDurationSeconds: 1200,
 * });
 * ```
 */
export function deriveCycleSegments(input: CycleSegmentsInput): CycleSegment[] {
  const elapsedRatio = activeSessionElapsedRatio(input);
  const isWorkSession = input.sessionType === SessionType.WORK;
  const segments: CycleSegment[] = [];

  for (
    let cycleNumber = 1;
    cycleNumber <= input.longBreakInterval;
    cycleNumber++
  ) {
    const isPastCycle = cycleNumber < input.currentCycle;
    const isCurrentCycle = cycleNumber === input.currentCycle;

    segments.push(
      buildWorkSegment(
        cycleNumber,
        resolveSegmentProgress(
          isPastCycle || (isCurrentCycle && !isWorkSession),
          isCurrentCycle && isWorkSession,
          elapsedRatio,
        ),
      ),
    );

    segments.push(
      buildBreakSegment(
        cycleNumber,
        cycleNumber === input.longBreakInterval,
        resolveSegmentProgress(
          isPastCycle,
          isCurrentCycle && !isWorkSession,
          elapsedRatio,
        ),
      ),
    );
  }

  return segments;
}
