/**
 * Progress Indicator Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Displays current step progress in the skip flow (e.g., "Step 3/10")
 * Visual indicator shows how many steps remain
 */

interface ProgressIndicatorProps {
  currentStep: number;
  totalSteps: number;
}

function getStepDotClassName(isCompleted: boolean, isCurrent: boolean): string {
  if (isCompleted) return "bg-primary-600";
  if (isCurrent) return "scale-125 bg-primary-500";
  return "bg-border-strong";
}

/**
 * Progress indicator showing current step in skip flow
 */
export default function ProgressIndicator({
  currentStep,
  totalSteps,
}: ProgressIndicatorProps): React.ReactElement {
  // Calculate progress percentage
  const progressPercentage = (currentStep / totalSteps) * 100;

  return (
    <div className="space-y-2">
      {/* Step counter text */}
      <div className="text-center text-sm font-medium text-text-muted">
        Step {currentStep} of {totalSteps}
      </div>

      {/* Progress bar */}
      <div className="h-2 w-full overflow-hidden rounded-full bg-surface-hover">
        <div
          className="h-full bg-primary-600 transition-all duration-300"
          role="progressbar"
          aria-valuenow={currentStep}
          aria-valuemin={1}
          aria-valuemax={totalSteps}
          aria-label={`Progress: ${String(Math.round(progressPercentage))}%`}
        >
          <svg width="100%" height="100%" xmlns="http://www.w3.org/2000/svg">
            <rect
              width={`${String(progressPercentage)}%`}
              height="100%"
              fill="currentColor"
            />
          </svg>
        </div>
      </div>

      {/* Step dots (optional visual indicator) */}
      <div className="flex justify-center space-x-2">
        {Array.from({ length: totalSteps }).map((_, index) => {
          const stepNumber = index + 1;
          const isCompleted = stepNumber < currentStep;
          const isCurrent = stepNumber === currentStep;

          return (
            <div
              key={stepNumber}
              className={`h-2 w-2 rounded-full transition-all duration-200 ${getStepDotClassName(isCompleted, isCurrent)}`}
              aria-label={`Step ${String(stepNumber)}`}
            />
          );
        })}
      </div>
    </div>
  );
}
