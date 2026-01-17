/**
 * Unit tests for mathGenerator utility
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect } from "vitest";
import {
  generateEasyProblem,
  generateHardProblem,
  validateAnswer,
} from "../mathGenerator";

describe("mathGenerator", () => {
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
        const numbers = problem.question.match(/\d+/g)?.map(Number);
        expect(numbers).toBeDefined();
        expect(numbers!.length).toBeGreaterThanOrEqual(2);

        // Check range (10-99 for easy problems)
        numbers!.forEach((num) => {
          expect(num).toBeGreaterThanOrEqual(10);
          expect(num).toBeLessThanOrEqual(99);
        });
      }
    });

    it("should generate addition problems", () => {
      let foundAddition = false;

      // Generate multiple problems to find addition
      for (let i = 0; i < 50; i++) {
        const problem = generateEasyProblem();
        if (problem.question.includes("+")) {
          foundAddition = true;

          // Verify addition is correct
          const [a, b] = problem.question.match(/\d+/g)!.map(Number);
          expect(problem.answer).toBe(a + b);
          break;
        }
      }

      expect(foundAddition).toBe(true);
    });

    it("should generate subtraction problems", () => {
      let foundSubtraction = false;

      // Generate multiple problems to find subtraction
      for (let i = 0; i < 50; i++) {
        const problem = generateEasyProblem();
        if (problem.question.includes("-")) {
          foundSubtraction = true;

          // Verify subtraction is correct
          const [a, b] = problem.question.match(/\d+/g)!.map(Number);
          expect(problem.answer).toBe(a - b);

          // Verify result is positive
          expect(problem.answer).toBeGreaterThan(0);
          break;
        }
      }

      expect(foundSubtraction).toBe(true);
    });

    it("should have correct answer for generated problems", () => {
      for (let i = 0; i < 20; i++) {
        const problem = generateEasyProblem();
        const numbers = problem.question.match(/\d+/g)!.map(Number);

        if (problem.question.includes("+")) {
          expect(problem.answer).toBe(numbers[0] + numbers[1]);
        } else if (problem.question.includes("-")) {
          expect(problem.answer).toBe(numbers[0] - numbers[1]);
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
      let foundMultiplication = false;

      // Generate multiple problems to find multiplication
      for (let i = 0; i < 50; i++) {
        const problem = generateHardProblem();
        if (problem.question.includes("×")) {
          foundMultiplication = true;

          // Verify multiplication is correct
          const [a, b] = problem.question.match(/\d+/g)!.map(Number);
          expect(problem.answer).toBe(a * b);

          // Check multiplication range (2-12)
          expect(a).toBeGreaterThanOrEqual(2);
          expect(a).toBeLessThanOrEqual(12);
          expect(b).toBeGreaterThanOrEqual(2);
          expect(b).toBeLessThanOrEqual(12);
          break;
        }
      }

      expect(foundMultiplication).toBe(true);
    });

    it("should generate large subtraction problems", () => {
      let foundSubtraction = false;

      // Generate multiple problems to find subtraction
      for (let i = 0; i < 50; i++) {
        const problem = generateHardProblem();
        if (problem.question.includes("-")) {
          foundSubtraction = true;

          // Verify subtraction is correct
          const [a, b] = problem.question.match(/\d+/g)!.map(Number);
          expect(problem.answer).toBe(a - b);

          // Check subtraction range (100-200 minus 20-80)
          expect(a).toBeGreaterThanOrEqual(100);
          expect(a).toBeLessThanOrEqual(200);
          expect(b).toBeGreaterThanOrEqual(20);
          expect(b).toBeLessThanOrEqual(80);
          break;
        }
      }

      expect(foundSubtraction).toBe(true);
    });

    it("should have correct answer for generated problems", () => {
      for (let i = 0; i < 20; i++) {
        const problem = generateHardProblem();
        const numbers = problem.question.match(/\d+/g)!.map(Number);

        if (problem.question.includes("×")) {
          expect(problem.answer).toBe(numbers[0] * numbers[1]);
        } else if (problem.question.includes("-")) {
          expect(problem.answer).toBe(numbers[0] - numbers[1]);
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
        expect(problem.question).toMatch(/^\d+ [+\-] \d+ = \?$/);
      }
    });

    it("hard problems should have correct format", () => {
      for (let i = 0; i < 10; i++) {
        const problem = generateHardProblem();
        expect(problem.question).toMatch(/^\d+ [×\-] \d+ = \?$/);
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
