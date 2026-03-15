/**
 * Skip Flow Modal Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Container component that manages the 10-step skip confirmation process.
 * Displays appropriate step component based on current step.
 */

import { useEffect } from "react";
import { useSkipFlowStore, SKIP_FLOW_STEPS } from "../../store/skipFlowStore";
import ProgressIndicator from "./ProgressIndicator";
import ConfirmationStep from "./steps/ConfirmationStep";
import TypingChallenge from "./steps/TypingChallenge";
import MathChallenge from "./steps/MathChallenge";
import LongPressButton from "./steps/LongPressButton";
import FinalWarning from "./steps/FinalWarning";

interface SkipFlowModalProps {
  onComplete: () => void;
  onCancel: () => void;
}

/**
 * Main skip flow modal component
 * Renders different step components based on current step
 *
 * Note: Input penalties are now handled entirely by the backend.
 * The backend monitors global input and applies penalties directly.
 */
export default function SkipFlowModal({ onComplete, onCancel }: SkipFlowModalProps) {
  const { currentStep, nextStep, cancelSkipFlow, completeSkipFlow, updateTimeSpent } =
    useSkipFlowStore();

  // Update time spent every second
  useEffect(() => {
    const interval = setInterval(() => {
      updateTimeSpent();
    }, 1000);

    return () => clearInterval(interval);
  }, [updateTimeSpent]);

  const handleNext = () => {
    nextStep();
  };

  const handleCancel = () => {
    cancelSkipFlow();
    onCancel();
  };

  const handleComplete = () => {
    completeSkipFlow();
    onComplete();
  };

  // Render appropriate step component
  const renderStep = () => {
    switch (currentStep) {
      case SKIP_FLOW_STEPS.CONFIRMATION_1:
      case SKIP_FLOW_STEPS.CONFIRMATION_2:
      case SKIP_FLOW_STEPS.CONFIRMATION_3:
      case SKIP_FLOW_STEPS.CONFIRMATION_4:
        return (
          <ConfirmationStep
            step={currentStep}
            onConfirm={handleNext}
            onCancel={handleCancel}
          />
        );

      case SKIP_FLOW_STEPS.TYPING_CHALLENGE_1:
        return (
          <TypingChallenge
            difficulty="short"
            onComplete={handleNext}
            onCancel={handleCancel}
          />
        );

      case SKIP_FLOW_STEPS.MATH_CHALLENGE_1:
        return (
          <MathChallenge
            difficulty="easy"
            onComplete={handleNext}
            onCancel={handleCancel}
          />
        );

      case SKIP_FLOW_STEPS.TYPING_CHALLENGE_2:
        return (
          <TypingChallenge
            difficulty="long"
            onComplete={handleNext}
            onCancel={handleCancel}
          />
        );

      case SKIP_FLOW_STEPS.MATH_CHALLENGE_2:
        return (
          <MathChallenge
            difficulty="hard"
            onComplete={handleNext}
            onCancel={handleCancel}
          />
        );

      case SKIP_FLOW_STEPS.LONG_PRESS:
        return (
          <LongPressButton onComplete={handleNext} onCancel={handleCancel} />
        );

      case SKIP_FLOW_STEPS.FINAL_WARNING:
        return (
          <FinalWarning onConfirm={handleComplete} onCancel={handleCancel} />
        );

      default:
        return null;
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-backdrop/90">
      <div className="w-full max-w-2xl rounded-lg border border-border bg-surface p-8 text-on-surface shadow-2xl shadow-shadow/40">
        {/* Progress Indicator */}
        <ProgressIndicator currentStep={currentStep} totalSteps={10} />

        {/* Step Content */}
        <div className="mt-8">{renderStep()}</div>
      </div>
    </div>
  );
}
