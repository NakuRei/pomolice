import { describe, expect, it } from "vitest";
import {
  getRandomMessage,
  motivationalMessages,
  selectRandomMotivationalMessage,
} from "./motivationalMessages";

describe("selectRandomMotivationalMessage", () => {
  it("returns one message from the given catalog", () => {
    const catalog = ["first", "second"] as const;

    expect(catalog).toContain(selectRandomMotivationalMessage(catalog));
  });

  it("rejects an empty catalog", () => {
    expect(() => selectRandomMotivationalMessage([])).toThrow();
  });
});

describe("getRandomMessage", () => {
  it("returns one configured motivational message", () => {
    expect(motivationalMessages).toContain(getRandomMessage());
  });
});
