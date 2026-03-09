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

import { useState, useEffect, useRef } from "react";
import { useSkipFlowStore } from "../../../store/skipFlowStore";
import {
  generateEasyProblem,
  generateHardProblem,
  validateAnswer,
  type MathProblem,
} from "../../../utils/mathGenerator";
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
}: MathChallengeProps) {
  const { retryStep } = useSkipFlowStore();
  const [problem, setProblem] = useState<MathProblem | null>(null);
  const [userAnswer, setUserAnswer] = useState("");
  const [showError, setShowError] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Generate initial problem
  useEffect(() => {
    generateNewProblem();
    // Focus input on mount
    inputRef.current?.focus();
  }, [difficulty]);

  const generateNewProblem = () => {
    const newProblem =
      difficulty === "easy" ? generateEasyProblem() : generateHardProblem();
    setProblem(newProblem);
    setUserAnswer("");
    setShowError(false);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();

    if (!problem || userAnswer.trim() === "") {
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

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    // Only allow numbers and negative sign
    const value = e.target.value;
    if (value === "" || value === "-" || /^-?\d+$/.test(value)) {
      setUserAnswer(value);
    }
  };

  if (!problem) {
    return <div>Loading...</div>;
  }

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
          <div className="rounded-lg bg-red-900 bg-opacity-50 p-4 text-center text-red-300">
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
      <div className="rounded-lg bg-gray-800 p-8 text-center">
        <div className="mb-4 font-mono text-4xl font-bold text-white">
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
              className={`w-32 rounded-lg border-2 bg-gray-900 px-4 py-3 text-center font-mono text-2xl text-white placeholder-gray-600 focus:outline-none focus:ring-2 ${
                showError
                  ? "border-red-500 focus:ring-red-500"
                  : "border-gray-700 focus:ring-blue-500"
              }`}
            />
            <button
              type="submit"
              disabled={showError || userAnswer.trim() === ""}
              className="rounded-lg bg-blue-600 px-6 py-3 font-semibold text-white transition-colors hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-gray-900 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Submit
            </button>
          </div>
        </form>
      </div>
    </SkipFlowStepFrame>
  );
}
