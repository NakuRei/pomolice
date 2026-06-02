/**
 * Unit tests for mathGenerator utility
 * Phase 5: Multi-step skip confirmation flow
 */

import { afterEach, describe, it, expect, vi } from "vitest";
import {
  generateEasyProblem,
  generateHardProblem,
  validateAnswer,
} from "./mathGenerator";

function parseQuestionOperands(question: string): [number, number] | null {
  const matches = question.match(/\d+/gu);
  if (matches === null || matches.length < 2) {
    return null;
  }

  const [firstOperand, secondOperand] = matches as [
    string,
    string,
    ...string[],
  ];

  return [Number(firstOperand), Number(secondOperand)];
}

function getQuestionOperands(question: string): [number, number] {
  const operands = parseQuestionOperands(question);
  expect(operands).not.toBeNull();
  if (operands === null) {
    throw new Error(`Expected two operands in question: ${question}`);
  }

  return operands;
}

describe("mathGenerator", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("generateEasyProblem", () => {
    it("should generate a problem with valid structure", () => {
      const problem = generateEasyProblem();

      expect(problem).toHaveProperty("question");
      expect(problem).toHaveProperty("answer");
      expect(typeof problem.question).toBe("string");
      expect(typeof problem.answer).toBe("number");
    });

    it("should generate problems with numbers in valid range", () => {
      // Test 20 times to ensure consistency
      for (let i = 0; i < 20; i++) {
        const problem = generateEasyProblem();

        // Parse numbers from question
        const numbers = problem.question.match(/\d+/gu)?.map(Number);
        expect(numbers).toBeDefined();
        if (numbers === undefined) continue;
        expect(numbers.length).toBeGreaterThanOrEqual(2);

        // Check range (10-99 for easy problems)
        numbers.forEach((num) => {
          expect(num).toBeGreaterThanOrEqual(10);
          expect(num).toBeLessThanOrEqual(99);
        });
      }
    });

    it("should generate addition problems", () => {
      vi.spyOn(Math, "random").mockReturnValue(0.4);

      const problem = generateEasyProblem();

      expect(problem.question).toContain("+");
      const [a, b] = getQuestionOperands(problem.question);
      expect(problem.answer).toBe(a + b);
    });

    it("should generate subtraction problems", () => {
      vi.spyOn(Math, "random").mockReturnValue(0.6);

      const problem = generateEasyProblem();

      expect(problem.question).toContain("-");
      const [a, b] = getQuestionOperands(problem.question);
      expect(problem.answer).toBe(a - b);
      expect(problem.answer).toBeGreaterThan(0);
    });

    it("should have correct answer for generated problems", () => {
      for (let i = 0; i < 20; i++) {
        const problem = generateEasyProblem();
        const operands = parseQuestionOperands(problem.question);
        expect(operands).not.toBeNull();
        if (operands === null) continue;
        const [a, b] = operands;

        if (problem.question.includes("+")) {
          expect(problem.answer).toBe(a + b);
        } else if (problem.question.includes("-")) {
          expect(problem.answer).toBe(a - b);
        }
      }
    });
  });

  describe("generateHardProblem", () => {
    it("should generate a problem with valid structure", () => {
      const problem = generateHardProblem();

      expect(problem).toHaveProperty("question");
      expect(problem).toHaveProperty("answer");
      expect(typeof problem.question).toBe("string");
      expect(typeof problem.answer).toBe("number");
    });

    it("should generate multiplication problems", () => {
      vi.spyOn(Math, "random").mockReturnValue(0.4);

      const problem = generateHardProblem();

      expect(problem.question).toContain("×");
      const [a, b] = getQuestionOperands(problem.question);
      expect(problem.answer).toBe(a * b);
      expect(a).toBeGreaterThanOrEqual(2);
      expect(a).toBeLessThanOrEqual(12);
      expect(b).toBeGreaterThanOrEqual(2);
      expect(b).toBeLessThanOrEqual(12);
    });

    it("should generate large subtraction problems", () => {
      vi.spyOn(Math, "random").mockReturnValue(0.6);

      const problem = generateHardProblem();

      expect(problem.question).toContain("-");
      const [a, b] = getQuestionOperands(problem.question);
      expect(problem.answer).toBe(a - b);
      expect(a).toBeGreaterThanOrEqual(100);
      expect(a).toBeLessThanOrEqual(200);
      expect(b).toBeGreaterThanOrEqual(20);
      expect(b).toBeLessThanOrEqual(80);
    });

    it("should have correct answer for generated problems", () => {
      for (let i = 0; i < 20; i++) {
        const problem = generateHardProblem();
        const operands = parseQuestionOperands(problem.question);
        expect(operands).not.toBeNull();
        if (operands === null) continue;
        const [a, b] = operands;

        if (problem.question.includes("×")) {
          expect(problem.answer).toBe(a * b);
        } else if (problem.question.includes("-")) {
          expect(problem.answer).toBe(a - b);
        }
      }
    });
  });

  describe("validateAnswer", () => {
    it("should return true for correct numeric answer", () => {
      expect(validateAnswer(42, 42)).toBe(true);
      expect(validateAnswer("42", 42)).toBe(true);
    });

    it("should return false for incorrect answer", () => {
      expect(validateAnswer(41, 42)).toBe(false);
      expect(validateAnswer("41", 42)).toBe(false);
    });

    it("should return false for invalid input", () => {
      expect(validateAnswer("abc", 42)).toBe(false);
      expect(validateAnswer("", 42)).toBe(false);
      expect(validateAnswer("12abc", 42)).toBe(false);
    });

    it("should handle negative numbers", () => {
      expect(validateAnswer(-5, -5)).toBe(true);
      expect(validateAnswer("-5", -5)).toBe(true);
      expect(validateAnswer(-5, 5)).toBe(false);
    });

    it("should handle zero", () => {
      expect(validateAnswer(0, 0)).toBe(true);
      expect(validateAnswer("0", 0)).toBe(true);
    });

    it("should handle string parsing correctly", () => {
      expect(validateAnswer("42", 42)).toBe(true);
      expect(validateAnswer("42 ", 42)).toBe(true); // parseInt ignores trailing whitespace
      expect(validateAnswer(" 42", 42)).toBe(true); // parseInt ignores leading whitespace
    });
  });

  describe("Problem format", () => {
    it("easy problems should have correct format", () => {
      for (let i = 0; i < 10; i++) {
        const problem = generateEasyProblem();
        expect(problem.question).toMatch(/^\d+ [+-] \d+ = \?$/u);
      }
    });

    it("hard problems should have correct format", () => {
      for (let i = 0; i < 10; i++) {
        const problem = generateHardProblem();
        expect(problem.question).toMatch(/^\d+ [×-] \d+ = \?$/u);
      }
    });
  });

  describe("Randomness", () => {
    it("should generate different problems", () => {
      const problems = new Set();

      for (let i = 0; i < 20; i++) {
        const problem = generateEasyProblem();
        problems.add(problem.question);
      }

      // Should have at least some variety (not all the same)
      expect(problems.size).toBeGreaterThan(1);
    });
  });
});
