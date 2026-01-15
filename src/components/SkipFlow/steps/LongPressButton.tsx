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
}: LongPressButtonProps) {
  const [isPressing, setIsPressing] = useState(false);
  const [progress, setProgress] = useState(0);
  const [hasReleased, setHasReleased] = useState(false);
  const startTimeRef = useRef<number>(0);
  const intervalRef = useRef<NodeJS.Timeout | null>(null);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, []);

  const startPress = () => {
    setIsPressing(true);
    setHasReleased(false);
    startTimeRef.current = Date.now();

    // Update progress continuously
    intervalRef.current = setInterval(() => {
      const elapsed = Date.now() - startTimeRef.current;
      const currentProgress = Math.min((elapsed / REQUIRED_DURATION) * 100, 100);
      setProgress(currentProgress);

      // Check if completed
      if (elapsed >= REQUIRED_DURATION) {
        endPress(true);
      }
    }, UPDATE_INTERVAL);
  };

  const endPress = (completed: boolean = false) => {
    setIsPressing(false);

    if (intervalRef.current) {
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
      setTimeout(() => {
        setHasReleased(false);
      }, 2000);
    }
  };

  const handleMouseDown = () => {
    startPress();
  };

  const handleMouseUp = () => {
    if (progress < 100) {
      endPress(false);
    }
  };

  const handleMouseLeave = () => {
    if (isPressing) {
      endPress(false);
    }
  };

  // Remaining time in seconds
  const remainingSeconds = Math.ceil(
    ((100 - progress) / 100) * (REQUIRED_DURATION / 1000)
  );

  return (
    <div className="space-y-8">
      {/* Instructions */}
      <div className="text-center">
        <h2 className="mb-2 text-2xl font-bold text-white">
          Long Press Challenge
        </h2>
        <p className="text-gray-400">
          Press and hold the button below for{" "}
          <strong>15 seconds</strong> without releasing.
        </p>
      </div>

      {/* Long press button with progress */}
      <div className="flex flex-col items-center space-y-4">
        <button
          onMouseDown={handleMouseDown}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseLeave}
          onTouchStart={handleMouseDown}
          onTouchEnd={handleMouseUp}
          className={`relative h-32 w-full overflow-hidden rounded-lg font-bold text-white shadow-lg transition-all focus:outline-none focus:ring-4 focus:ring-offset-2 focus:ring-offset-gray-900 ${
            isPressing
              ? "bg-blue-600 focus:ring-blue-500"
              : "bg-gray-700 hover:bg-gray-600 focus:ring-gray-500"
          }`}
        >
          {/* Progress bar background */}
          <div
            className="absolute inset-0 bg-blue-500 transition-all"
            style={{ width: `${progress}%` }}
          />

          {/* Button text */}
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

        {/* Progress percentage */}
        <div className="text-center text-sm text-gray-400">
          Progress: {Math.floor(progress)}%
        </div>
      </div>

      {/* Release warning */}
      {hasReleased && (
        <div className="rounded-lg bg-red-900 bg-opacity-50 p-4 text-center text-red-300">
          <p className="font-semibold">You released too early! Try again.</p>
        </div>
      )}

      {/* Hint */}
      <div className="text-center text-sm text-gray-500">
        <p>Tip: Keep your finger or mouse button pressed continuously.</p>
        <p className="mt-1">
          Releasing before 15 seconds will restart the timer.
        </p>
      </div>

      {/* Cancel button */}
      <div className="flex justify-center">
        <button
          onClick={onCancel}
          className="rounded-lg bg-gray-700 px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-600 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 focus:ring-offset-gray-900"
        >
          Cancel and Take Break
        </button>
      </div>
    </div>
  );
}
