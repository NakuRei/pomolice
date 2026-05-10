/**
 * Long Press Button Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Step 9: Hold button for 15 seconds
 * Features:
 * - Visual progress bar
 * - Must hold continuously
 * - Releasing restarts the timer
 */

import { useState, useRef, useEffect } from "react";
import SkipFlowCancelButton from "../SkipFlowCancelButton";
import SkipFlowStepFrame from "../SkipFlowStepFrame";

interface LongPressButtonProps {
  onComplete: () => void;
  onCancel: () => void;
}

const REQUIRED_DURATION = 15000; // 15 seconds in milliseconds
const UPDATE_INTERVAL = 50; // Update progress every 50ms

/**
 * Long press button component with progress tracking
 */
export default function LongPressButton({
  onComplete,
  onCancel,
}: LongPressButtonProps): React.ReactElement {
  const [isPressing, setIsPressing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [hasReleased, setHasReleased] = useState(false);
  const elapsedMsRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const releaseResetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const pressButtonReference = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    pressButtonReference.current?.focus();
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return (): void => {
      if (intervalRef.current !== null) {
        clearInterval(intervalRef.current);
      }
      if (releaseResetTimeoutRef.current !== null) {
        clearTimeout(releaseResetTimeoutRef.current);
      }
    };
  }, []);

  const endPress = (completed = false): void => {
    setIsPressing(false);

    if (intervalRef.current !== null) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }

    if (completed) {
      // Successfully held for required duration
      setProgress(100);
      onComplete();
    } else {
      // Released early - reset
      setProgress(0);
      setHasReleased(true);

      // Clear "released" message after 2 seconds
      releaseResetTimeoutRef.current = setTimeout(() => {
        setHasReleased(false);
        releaseResetTimeoutRef.current = null;
      }, 2000);
    }
  };

  const startPress = (): void => {
    setIsPressing(true);
    setHasReleased(false);
    elapsedMsRef.current = 0;

    // Update progress continuously
    intervalRef.current = setInterval(() => {
      elapsedMsRef.current += UPDATE_INTERVAL;
      const elapsed = elapsedMsRef.current;
      const currentProgress = Math.min(
        (elapsed / REQUIRED_DURATION) * 100,
        100,
      );
      setProgress(currentProgress);

      // Check if completed
      if (elapsed >= REQUIRED_DURATION) {
        endPress(true);
      }
    }, UPDATE_INTERVAL);
  };

  const handleMouseDown = (): void => {
    startPress();
  };

  const handleMouseUp = (): void => {
    if (progress < 100) {
      endPress(false);
    }
  };

  const handleMouseLeave = (): void => {
    if (isPressing) {
      endPress(false);
    }
  };

  // Remaining time in seconds
  const remainingSeconds = Math.ceil(
    ((100 - progress) / 100) * (REQUIRED_DURATION / 1000),
  );

  return (
    <SkipFlowStepFrame
      title="Long Press Challenge"
      description={
        <>
          Press and hold the button below for <strong>15 seconds</strong>{" "}
          without releasing.
        </>
      }
      notice={
        hasReleased ? (
          <div className="rounded-lg bg-danger-950/50 p-4 text-center text-danger-300">
            <p className="font-semibold">You released too early! Try again.</p>
          </div>
        ) : null
      }
      footer={
        <>
          <p>Tip: Keep your finger or mouse button pressed continuously.</p>
          <p className="mt-1">
            Releasing before 15 seconds will restart the timer.
          </p>
        </>
      }
      actions={
        <div className="flex justify-center">
          <SkipFlowCancelButton onCancel={onCancel} />
        </div>
      }
    >
      <div className="flex flex-col items-center space-y-4">
        <button
          ref={pressButtonReference}
          onMouseDown={handleMouseDown}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseLeave}
          onTouchStart={handleMouseDown}
          onTouchEnd={handleMouseUp}
          className={`relative h-32 w-full overflow-hidden rounded-lg font-bold shadow-lg transition-all focus:ring-4 focus:ring-offset-2 focus:ring-offset-surface focus:outline-none ${
            isPressing
              ? "bg-primary-700 text-on-primary focus:ring-primary-600"
              : "bg-surface-raised text-on-surface hover:bg-surface-hover focus:ring-primary-600"
          }`}
        >
          <div className="absolute inset-0 overflow-hidden text-primary-700">
            <svg
              className="absolute inset-0 h-full w-full"
              xmlns="http://www.w3.org/2000/svg"
              preserveAspectRatio="none"
            >
              <rect
                width={`${String(progress)}%`}
                height="100%"
                fill="currentColor"
                className="transition-all"
              />
            </svg>
          </div>

          <div className="relative z-10 flex h-full items-center justify-center">
            {isPressing ? (
              <div className="text-center">
                <div className="text-3xl">{remainingSeconds}s</div>
                <div className="text-sm">Keep holding...</div>
              </div>
            ) : (
              <div className="text-xl">Press and Hold</div>
            )}
          </div>
        </button>

        <div className="text-center text-sm text-text-muted">
          Progress: {Math.floor(progress)}%
        </div>
      </div>
    </SkipFlowStepFrame>
  );
}
