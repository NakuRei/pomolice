import { describe, expect, it, vi } from "vitest";
import { receiveCommandResponse } from "./receiveCommandResponse";

describe("command response delivery", () => {
  it("preserves a delivered application rejection", async () => {
    const response = {
      status: "rejected",
      error: { code: "rate_limited" },
    } as const;
    expect(await receiveCommandResponse(Promise.resolve(response))).toEqual(
      response,
    );
  });

  it("never interprets a rejected promise as a delivered rejection", async () => {
    const cause = { code: "rate_limited" };
    const request = vi.fn<() => Promise<never>>().mockRejectedValue(cause);
    expect(await receiveCommandResponse(request())).toEqual({
      status: "unavailable",
      cause,
    });
  });
});
