/**
 * Math Challenge Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Steps 6 & 8: Solve math problems to proceed
 * Features:
 * - Easy: Addition and subtraction (10-99)
 * - Hard: Multiplication and larger subtraction
 * - New problem on incorrect answer
 * - Retry counting
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import {
  generateEasyProblem,
  generateHardProblem,
  validateAnswer,
  type MathProblem,
} from "@/utils/mathGenerator";
import SkipFlowCancelButton from "../SkipFlowCancelButton";
import SkipFlowStepFrame from "../SkipFlowStepFrame";

interface MathChallengeProps {
  difficulty: "easy" | "hard";
  onComplete: () => void;
  onCancel: () => void;
}

/**
 * Math challenge component with problem generation
 */
export default function MathChallenge({
  difficulty,
  onComplete,
  onCancel,
}: MathChallengeProps): React.ReactElement {
  const retryStep = useSkipFlowStore((state) => state.retryStep);
  const [problem, setProblem] = useState<MathProblem>(() =>
    difficulty === "easy" ? generateEasyProblem() : generateHardProblem(),
  );
  const [userAnswer, setUserAnswer] = useState("");
  const [showError, setShowError] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const generateNewProblem = useCallback(() => {
    const newProblem =
      difficulty === "easy" ? generateEasyProblem() : generateHardProblem();
    setProblem(newProblem);
    setUserAnswer("");
    setShowError(false);
  }, [difficulty]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = (e: React.SyntheticEvent<HTMLFormElement>): void => {
    e.preventDefault();

    if (userAnswer.trim() === "") {
      return;
    }

    const isCorrect = validateAnswer(userAnswer, problem.answer);

    if (isCorrect) {
      // Correct answer - proceed to next step
      onComplete();
    } else {
      // Incorrect answer - show error and generate new problem
      setShowError(true);
      retryStep();

      // Generate new problem after brief delay
      setTimeout(() => {
        generateNewProblem();
        inputRef.current?.focus();
      }, 1500);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
    // Only allow numbers and negative sign
    const value = e.target.value;
    if (value === "" || value === "-" || /^-?\d+$/u.test(value)) {
      setUserAnswer(value);
    }
  };

  return (
    <SkipFlowStepFrame
      title="Math Challenge"
      description={
        <>
          Solve the following {difficulty === "easy" ? "simple" : "challenging"}{" "}
          math problem:
        </>
      }
      notice={
        showError ? (
          <div className="rounded-lg bg-danger-950/50 p-4 text-center text-danger-300">
            <p className="font-semibold">
              Incorrect! Generating new problem...
            </p>
          </div>
        ) : null
      }
      footer={
        <>
          <p>Tip: Take your time to calculate correctly.</p>
          <p className="mt-1">Wrong answers will generate a new problem.</p>
        </>
      }
      actions={
        <div className="flex justify-center">
          <SkipFlowCancelButton onCancel={onCancel} />
        </div>
      }
    >
      <div className="rounded-lg bg-surface-raised p-8 text-center">
        <div className="mb-4 font-mono text-4xl font-bold text-text">
          {problem.question}
        </div>

        <form onSubmit={handleSubmit} className="mt-6">
          <div className="flex items-center justify-center space-x-4">
            <input
              ref={inputRef}
              type="text"
              inputMode="numeric"
              value={userAnswer}
              onChange={handleInputChange}
              disabled={showError}
              placeholder="Your answer"
              autoComplete="off"
              className={`w-32 rounded-lg border-2 bg-background px-4 py-3 text-center font-mono text-2xl text-on-background placeholder:text-text-muted focus:ring-2 focus:outline-none ${
                showError
                  ? "border-danger-500 focus:ring-danger-500"
                  : "border-border focus:ring-primary-600"
              }`}
            />
            <button
              type="submit"
              disabled={showError || userAnswer.trim() === ""}
              className="rounded-lg bg-primary-700 px-6 py-3 font-semibold text-on-primary transition-colors hover:bg-primary-800 focus:ring-2 focus:ring-primary-600 focus:ring-offset-2 focus:ring-offset-surface focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
            >
              Submit
            </button>
          </div>
        </form>
      </div>
    </SkipFlowStepFrame>
  );
}
