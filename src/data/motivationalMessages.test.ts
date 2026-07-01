import { describe, expect, it } from "vitest";
import {
  breakMotivationalMessages,
  getRandomBreakMotivationalMessage,
} from "./motivationalMessages";

describe("getRandomBreakMotivationalMessage", () => {
  it("returns one configured motivational message", () => {
    expect(breakMotivationalMessages).toContain(
      getRandomBreakMotivationalMessage(),
    );
  });
});
