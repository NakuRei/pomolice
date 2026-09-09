import { afterEach, describe, it, expect, vi } from "vitest";
import {
  generateEasyProblem,
  generateHardProblem,
  validateAnswer,
} from "./mathGenerator";

function parseQuestion(question: string): {
  left: number;
  operator: string;
  right: number;
} {
  const groups = /^(?<left>\d+) (?<operator>[+×-]) (?<right>\d+) = \?$/u.exec(
    question,
  )?.groups;
  if (groups?.operator === undefined) {
    throw new Error(`Invalid math question: ${question}`);
  }

  return {
    left: Number(groups.left),
    operator: groups.operator,
    right: Number(groups.right),
  };
}

describe("mathGenerator", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("generates solvable addition and subtraction problems within the easy range", () => {
    const random = vi.spyOn(Math, "random");
    const operators = new Set<string>();

    for (const randomValue of [0, 0.25, 0.75, 0.999999]) {
      random.mockReturnValue(randomValue);

      const problem = generateEasyProblem();
      const { left, operator, right } = parseQuestion(problem.question);

      operators.add(operator);
      if (operator === "+") {
        expect(left).toBeGreaterThanOrEqual(10);
        expect(left).toBeLessThanOrEqual(99);
        expect(right).toBeGreaterThanOrEqual(10);
        expect(right).toBeLessThanOrEqual(99);
        expect(problem.answer).toBe(left + right);
      } else {
        expect(left).toBeGreaterThanOrEqual(20);
        expect(left).toBeLessThanOrEqual(99);
        expect(right).toBeGreaterThanOrEqual(10);
        expect(right).toBeLessThanOrEqual(50);
        expect(problem.answer).toBe(left - right);
        expect(problem.answer).toBeGreaterThan(0);
      }
    }

    expect(operators).toEqual(new Set(["+", "-"]));
  });

  it("generates solvable multiplication and subtraction problems within the hard range", () => {
    const random = vi.spyOn(Math, "random");
    const operators = new Set<string>();

    for (const randomValue of [0, 0.25, 0.75, 0.999999]) {
      random.mockReturnValue(randomValue);

      const problem = generateHardProblem();
      const { left, operator, right } = parseQuestion(problem.question);

      operators.add(operator);
      if (operator === "×") {
        expect(left).toBeGreaterThanOrEqual(2);
        expect(left).toBeLessThanOrEqual(12);
        expect(right).toBeGreaterThanOrEqual(2);
        expect(right).toBeLessThanOrEqual(12);
        expect(problem.answer).toBe(left * right);
      } else {
        expect(left).toBeGreaterThanOrEqual(100);
        expect(left).toBeLessThanOrEqual(200);
        expect(right).toBeGreaterThanOrEqual(20);
        expect(right).toBeLessThanOrEqual(80);
        expect(problem.answer).toBe(left - right);
      }
    }

    expect(operators).toEqual(new Set(["×", "-"]));
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

    it("should accept surrounding whitespace", () => {
      expect(validateAnswer("42", 42)).toBe(true);
      expect(validateAnswer("42 ", 42)).toBe(true);
      expect(validateAnswer(" 42", 42)).toBe(true);
    });
  });
});
