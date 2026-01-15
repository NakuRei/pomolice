/**
 * Math problem generator for skip flow challenges
 * Phase 5: Multi-step skip confirmation
 *
 * Two difficulty levels:
 * - Easy: Step 6 (addition and subtraction with numbers 10-99)
 * - Hard: Step 8 (multiplication and larger subtraction)
 */

/**
 * Math problem type
 */
export interface MathProblem {
  question: string;
  answer: number;
}

/**
 * Operation types for math problems
 */
export enum MathOperation {
  ADD = "add",
  SUBTRACT = "subtract",
  MULTIPLY = "multiply",
}

/**
 * Generate a random integer between min and max (inclusive)
 * @param min - Minimum value
 * @param max - Maximum value
 * @returns Random integer
 */
function randomInt(min: number, max: number): number {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

/**
 * Generate an easy math problem (step 6)
 * - Addition: 10-99 + 10-99
 * - Subtraction: 20-99 - 10-50 (ensuring positive result)
 *
 * @returns MathProblem object with question and answer
 */
export function generateEasyProblem(): MathProblem {
  const operation = Math.random() < 0.5 ? MathOperation.ADD : MathOperation.SUBTRACT;

  if (operation === MathOperation.ADD) {
    const a = randomInt(10, 99);
    const b = randomInt(10, 99);
    return {
      question: `${a} + ${b} = ?`,
      answer: a + b,
    };
  } else {
    // Subtraction: ensure positive result
    const a = randomInt(20, 99);
    const b = randomInt(10, Math.min(50, a - 1)); // b must be less than a
    return {
      question: `${a} - ${b} = ?`,
      answer: a - b,
    };
  }
}

/**
 * Generate a hard math problem (step 8)
 * - Multiplication: 2-12 × 2-12 (times tables)
 * - Large subtraction: 100-200 - 20-80
 *
 * @returns MathProblem object with question and answer
 */
export function generateHardProblem(): MathProblem {
  const operation = Math.random() < 0.5 ? MathOperation.MULTIPLY : MathOperation.SUBTRACT;

  if (operation === MathOperation.MULTIPLY) {
    const a = randomInt(2, 12);
    const b = randomInt(2, 12);
    return {
      question: `${a} × ${b} = ?`,
      answer: a * b,
    };
  } else {
    // Large subtraction
    const a = randomInt(100, 200);
    const b = randomInt(20, 80);
    return {
      question: `${a} - ${b} = ?`,
      answer: a - b,
    };
  }
}

/**
 * Validate user answer against correct answer
 * @param userAnswer - User's input (string or number)
 * @param correctAnswer - Correct answer
 * @returns True if correct, false otherwise
 */
export function validateAnswer(userAnswer: string | number, correctAnswer: number): boolean {
  const parsed = typeof userAnswer === "string" ? parseInt(userAnswer, 10) : userAnswer;

  // Check if valid number
  if (isNaN(parsed)) {
    return false;
  }

  return parsed === correctAnswer;
}
