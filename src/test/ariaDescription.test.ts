import { describe, expect, it } from "vitest";
import { getAriaDescribedByIds } from "./ariaDescription";

describe("getAriaDescribedByIds", () => {
  it("returns an empty array when aria-describedby is absent or empty", () => {
    const element = document.createElement("input");

    expect(getAriaDescribedByIds(element)).toEqual([]);

    element.setAttribute("aria-describedby", "");

    expect(getAriaDescribedByIds(element)).toEqual([]);
  });

  it("returns each non-empty described-by id in order", () => {
    const element = document.createElement("input");

    element.setAttribute("aria-describedby", "hint  error");

    expect(getAriaDescribedByIds(element)).toEqual(["hint", "error"]);
  });
});
