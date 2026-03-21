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

import { useEffect, useMemo, useRef, useState } from "react";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import {
  getRandomShortPhrase,
  getRandomLongPhrase,
  validateTypedPhrase,
  getCharacterValidation,
} from "@/utils/phraseGenerator";
import SkipFlowCancelButton from "../SkipFlowCancelButton";
import SkipFlowStepFrame from "../SkipFlowStepFrame";

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
}: TypingChallengeProps): React.ReactElement {
  const retryStep = useSkipFlowStore((state) => state.retryStep);
  const [userInput, setUserInput] = useState("");
  const [hasTypo, setHasTypo] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const resetTimeoutRef = useRef<number | null>(null);
  const targetPhrase = useMemo(
    () =>
      difficulty === "short" ? getRandomShortPhrase() : getRandomLongPhrase(),
    [difficulty],
  );

  useEffect(() => {
    inputRef.current?.focus();
  }, [targetPhrase]);

  useEffect(() => {
    return (): void => {
      if (resetTimeoutRef.current !== null) {
        clearTimeout(resetTimeoutRef.current);
      }
    };
  }, []);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
    if (hasTypo) {
      return;
    }

    const nextValue = e.target.value;
    setUserInput(nextValue);

    if (nextValue.length === 0) {
      return;
    }

    const validation = getCharacterValidation(nextValue, targetPhrase);
    const hasError = validation.some((result) => result === false);

    if (hasError) {
      setHasTypo(true);
      retryStep();

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

    if (validateTypedPhrase(nextValue, targetPhrase)) {
      onComplete();
    }
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLInputElement>): void => {
    e.preventDefault();
  };

  const handleDrop = (e: React.DragEvent<HTMLInputElement>): void => {
    e.preventDefault();
  };

  // Get character validation for styling
  const characterValidation = getCharacterValidation(userInput, targetPhrase);

  return (
    <SkipFlowStepFrame
      title="Typing Challenge"
      description={
        <>
          Type the following phrase <strong>exactly</strong> as shown:
        </>
      }
      notice={
        hasTypo ? (
          <div className="rounded-lg bg-danger-950/50 p-4 text-center text-danger-300">
            <p className="font-semibold">Typo detected! Starting over...</p>
          </div>
        ) : null
      }
      footer={
        <>
          <p>Tip: Type carefully. Any typo will restart this step.</p>
          <p className="mt-1">Case-sensitive. All letters must be UPPERCASE.</p>
        </>
      }
      actions={
        <div className="flex justify-center">
          <SkipFlowCancelButton onCancel={onCancel} />
        </div>
      }
    >
      <div className="rounded-lg bg-surface-raised p-6">
        <div className="font-mono text-lg tracking-wider">
          {Array.from(targetPhrase).map((char, index) => {
            const validation = characterValidation[index];
            let colorClass = "text-text-muted";

            if (validation === true) {
              colorClass = "text-success-400";
            } else if (validation === false) {
              colorClass = "text-danger-400";
            }

            return (
              <span key={index} className={colorClass}>
                {char}
              </span>
            );
          })}
        </div>
      </div>

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
          className={`w-full rounded-lg border-2 bg-background px-4 py-3 font-mono text-lg text-on-background placeholder:text-text-muted focus:ring-2 focus:outline-none ${
            hasTypo
              ? "border-danger-500 focus:ring-danger-500"
              : "border-border focus:ring-primary-600"
          }`}
        />
      </div>
    </SkipFlowStepFrame>
  );
}
