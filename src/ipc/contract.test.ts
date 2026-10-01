import { useBackendObservationStore } from "@/backend/observationStore";
import {
  type commands,
  type CommandResponse,
  type BackendErrorResponse,
  type SettingsValidationIssue,
  type SettingsChanges,
  type TimerSnapshot,
} from "./generated";
import { describe, expect, expectTypeOf, it } from "vitest";
import { createTimerEffectsForTest } from "@/test/timerEffectsFixture";

describe("IPC contract", () => {
  it("keeps omitted settings distinct from null and infers command responses", () => {
    expectTypeOf<
      Extract<BackendErrorResponse, { code: "invalid_settings" }>["issues"]
    >().toEqualTypeOf<SettingsValidationIssue[]>();
    expectTypeOf<SettingsChanges["notification_sound_muted"]>().toEqualTypeOf<
      boolean | undefined
    >();
    expectTypeOf<
      Parameters<typeof commands.resolveBootAutoStartPrompt>
    >().toEqualTypeOf<[boolean]>();
    expectTypeOf<ReturnType<typeof commands.getFullState>>().toEqualTypeOf<
      Promise<CommandResponse<TimerSnapshot>>
    >();
  });

  it.each([
    ["9", "10"],
    ["9007199254740992", "9007199254740993"],
    ["18446744073709551614", "18446744073709551615"],
  ])("orders revisions %s and %s without losing precision", (older, newer) => {
    useBackendObservationStore.setState({ effects: null });
    const previous = createTimerEffectsForTest({ revision: older });
    const latest = createTimerEffectsForTest({
      revision: newer,
      last_timer_save: "failed",
    });
    const { observeEffects } = useBackendObservationStore.getState();
    observeEffects(previous);
    observeEffects(latest);
    observeEffects(previous);
    expect(useBackendObservationStore.getState().effects).toEqual(latest);
  });
});
