/**
 * Typing Challenge Component
 * Phase 5: Multi-step skip confirmation flow
 *
 * Steps 5 & 7: Type exact phrases to proceed
 * Features:
 * - Real-time character-by-character validation
 * - Visual feedback (green = correct, red = incorrect)
 * - Case-sensitive matching (all phrases are UPPERCASE)
 * - Typo detection with instant restart
 */

import { useState, useEffect, useRef } from "react";
import { useSkipFlowStore } from "../../../store/skipFlowStore";
import {
  getRandomShortPhrase,
  getRandomLongPhrase,
  validateTypedPhrase,
  getCharacterValidation,
} from "../../../utils/phraseGenerator";

interface TypingChallengeProps {
  difficulty: "short" | "long";
  onComplete: () => void;
  onCancel: () => void;
}

/**
 * Typing challenge component with real-time validation
 */
export default function TypingChallenge({
  difficulty,
  onComplete,
  onCancel,
}: TypingChallengeProps) {
  const { retryStep } = useSkipFlowStore();
  const [targetPhrase, setTargetPhrase] = useState("");
  const [userInput, setUserInput] = useState("");
  const [hasTypo, setHasTypo] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const resetTimeoutRef = useRef<number | null>(null);

  // Generate target phrase on mount
  useEffect(() => {
    const phrase =
      difficulty === "short" ? getRandomShortPhrase() : getRandomLongPhrase();
    setTargetPhrase(phrase);
    // Focus input on mount
    inputRef.current?.focus();
  }, [difficulty]);

  // Validate input in real-time
  useEffect(() => {
    if (userInput.length === 0) {
      if (hasTypo) {
        setHasTypo(false);
      }
      return;
    }

    if (hasTypo) {
      return;
    }

    // Check if current input matches target so far
    const validation = getCharacterValidation(userInput, targetPhrase);
    const hasError = validation.some((result) => result === false);

    if (hasError) {
      setHasTypo(true);
      // Increment retry count
      retryStep();
      // Reset after showing error briefly
      if (resetTimeoutRef.current !== null) {
        clearTimeout(resetTimeoutRef.current);
      }
      resetTimeoutRef.current = window.setTimeout(() => {
        setUserInput("");
        setHasTypo(false);
        inputRef.current?.focus();
        resetTimeoutRef.current = null;
      }, 1000);
      return;
    }

    // Check if complete
    if (validateTypedPhrase(userInput, targetPhrase)) {
      onComplete();
    }
  }, [userInput, targetPhrase, hasTypo, onComplete, retryStep]);

  useEffect(() => {
    return () => {
      if (resetTimeoutRef.current !== null) {
        clearTimeout(resetTimeoutRef.current);
      }
    };
  }, []);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (hasTypo) return; // Prevent input during error display
    setUserInput(e.target.value);
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    e.preventDefault();
  };

  const handleDrop = (e: React.DragEvent<HTMLInputElement>) => {
    e.preventDefault();
  };

  // Get character validation for styling
  const characterValidation = getCharacterValidation(userInput, targetPhrase);

  return (
    <div className="space-y-8">
      {/* Instructions */}
      <div className="text-center">
        <h2 className="mb-2 text-2xl font-bold text-white">
          Typing Challenge
        </h2>
        <p className="text-gray-400">
          Type the following phrase <strong>exactly</strong> as shown:
        </p>
      </div>

      {/* Target phrase with character highlighting */}
      <div className="rounded-lg bg-gray-800 p-6">
        <div className="font-mono text-lg tracking-wider">
          {Array.from(targetPhrase).map((char, index) => {
            const validation = characterValidation[index];
            let colorClass = "text-gray-400";

            if (validation === true) {
              colorClass = "text-green-400";
            } else if (validation === false) {
              colorClass = "text-red-400";
            }

            return (
              <span key={index} className={colorClass}>
                {char}
              </span>
            );
          })}
        </div>
      </div>

      {/* Input field */}
      <div>
        <input
          ref={inputRef}
          type="text"
          value={userInput}
          onChange={handleInputChange}
          onPaste={handlePaste}
          onDrop={handleDrop}
          disabled={hasTypo}
          placeholder="Start typing here..."
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck="false"
          className={`w-full rounded-lg border-2 bg-gray-900 px-4 py-3 font-mono text-lg text-white placeholder-gray-600 focus:outline-none focus:ring-2 ${
            hasTypo
              ? "border-red-500 focus:ring-red-500"
              : "border-gray-700 focus:ring-blue-500"
          }`}
        />
      </div>

      {/* Error message */}
      {hasTypo && (
        <div className="rounded-lg bg-red-900 bg-opacity-50 p-4 text-center text-red-300">
          <p className="font-semibold">Typo detected! Starting over...</p>
        </div>
      )}

      {/* Hint */}
      <div className="text-center text-sm text-gray-500">
        <p>Tip: Type carefully. Any typo will restart this step.</p>
        <p className="mt-1">Case-sensitive. All letters must be UPPERCASE.</p>
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
