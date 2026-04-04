import { describe, it, expect } from "vitest";
import {
  getCharacterValidation,
  getConfirmationMessage,
  getRandomLongPhrase,
  getRandomShortPhrase,
  validateTypedPhrase,
} from "./phraseGenerator";
import {
  CONFIRMATION_MESSAGES,
  LONG_TYPING_PHRASES,
  SHORT_TYPING_PHRASES,
} from "@/data/typingPhrases";

describe("phraseGenerator", () => {
  it("returns a configured short phrase", () => {
    expect(SHORT_TYPING_PHRASES).toContain(getRandomShortPhrase());
  });

  it("returns a configured long phrase", () => {
    expect(LONG_TYPING_PHRASES).toContain(getRandomLongPhrase());
  });

  it("clamps confirmation steps to the supported message range", () => {
    expect(getConfirmationMessage(0)).toBe(CONFIRMATION_MESSAGES[0]);
    expect(getConfirmationMessage(2)).toBe(CONFIRMATION_MESSAGES[1]);
    expect(getConfirmationMessage(99)).toBe(
      CONFIRMATION_MESSAGES[CONFIRMATION_MESSAGES.length - 1],
    );
  });

  it("validates typed phrases with exact matching", () => {
    expect(validateTypedPhrase("TEST PHRASE", "TEST PHRASE")).toBe(true);
    expect(validateTypedPhrase("test phrase", "TEST PHRASE")).toBe(false);
    expect(validateTypedPhrase("TEST", "TEST PHRASE")).toBe(false);
  });

  it("returns per-character validation including untouched positions", () => {
    expect(getCharacterValidation("TEQ", "TEST")).toEqual([
      true,
      true,
      false,
      null,
    ]);
  });
});
