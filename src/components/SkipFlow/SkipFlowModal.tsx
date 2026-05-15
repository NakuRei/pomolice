/**
 * Skip Flow Modal Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Container component that manages the 10-step skip confirmation process.
 * Displays appropriate step component based on current step.
 */

import { useEffect, useId } from "react";
import { useSkipFlowStore, SKIP_FLOW_STEPS } from "@/store/skipFlowStore";
import ProgressIndicator from "./ProgressIndicator";
import ConfirmationStep from "./steps/ConfirmationStep";
import TypingChallenge from "./steps/TypingChallenge";
import MathChallenge from "./steps/MathChallenge";
import LongPressButton from "./steps/LongPressButton";
import FinalWarning from "./steps/FinalWarning";
import { DialogOverlay } from "../shared/DialogOverlay";
import { DialogPanel } from "../shared/DialogPanel";

const CONFIRMATION_STEPS = new Set<number>([
  SKIP_FLOW_STEPS.CONFIRMATION_1,
  SKIP_FLOW_STEPS.CONFIRMATION_2,
  SKIP_FLOW_STEPS.CONFIRMATION_3,
  SKIP_FLOW_STEPS.CONFIRMATION_4,
]);

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
export default function SkipFlowModal({
  onComplete,
  onCancel,
}: SkipFlowModalProps): React.ReactElement {
  const titleId = useId();
  const descriptionId = useId();
  const currentStep = useSkipFlowStore((state) => state.currentStep);
  const nextStep = useSkipFlowStore((state) => state.nextStep);
  const cancelSkipFlow = useSkipFlowStore((state) => state.cancelSkipFlow);
  const completeSkipFlow = useSkipFlowStore((state) => state.completeSkipFlow);
  const updateTimeSpent = useSkipFlowStore((state) => state.updateTimeSpent);

  // Update time spent every second
  useEffect(() => {
    const interval = setInterval(() => {
      updateTimeSpent();
    }, 1000);

    return (): void => {
      clearInterval(interval);
    };
  }, [updateTimeSpent]);

  const handleNext = (): void => {
    nextStep();
  };

  const handleCancel = (): void => {
    cancelSkipFlow();
    onCancel();
  };

  const handleComplete = (): void => {
    completeSkipFlow();
    onComplete();
  };

  // Render appropriate step component
  const renderStep = (): React.ReactElement | null => {
    if (CONFIRMATION_STEPS.has(currentStep)) {
      return (
        <ConfirmationStep
          step={currentStep}
          onConfirm={handleNext}
          onCancel={handleCancel}
        />
      );
    }

    switch (currentStep) {
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
    <DialogOverlay className="bg-backdrop/90">
      <DialogPanel
        labelledBy={titleId}
        describedBy={descriptionId}
        className="max-w-2xl shadow-2xl shadow-shadow/40"
      >
        <h2 id={titleId} className="sr-only">
          休憩スキップ確認
        </h2>
        <p id={descriptionId} className="sr-only">
          休憩をスキップするには、この確認フローを順番に完了してください。各ステップの指示に従って進めてください。
        </p>
        {/* Progress Indicator */}
        <ProgressIndicator currentStep={currentStep} totalSteps={10} />

        {/* Step Content */}
        <div className="mt-8">{renderStep()}</div>
      </DialogPanel>
    </DialogOverlay>
  );
}
