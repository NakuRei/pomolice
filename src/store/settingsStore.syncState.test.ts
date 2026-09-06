import { beforeEach, describe, expect, it } from "vitest";
import { resetSettingsStoreForTest } from "@/test/resetSettingsStore";
import {
  DEFAULT_SETTINGS,
  WEEKDAY_KEYS,
  createAutomationSettingsCandidate,
  createDurationSettingsCandidate,
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
  type WeeklyAutoRunConfig,
} from "./settingsStore";

type CandidateGroup = "duration" | "automation";
type ResetAction = "resetToDefaults" | "reset";

interface CandidateMutationCase {
  label: string;
  group: CandidateGroup;
  mutate: () => void;
}

interface ResetMutationCase {
  resetAction: ResetAction;
  changedGroup: CandidateGroup;
  prepare: () => void;
}

interface PersistedSettingsSnapshot {
  storageKey: string;
  serializedState: string;
}

function capturePersistedSettingsSnapshot(): PersistedSettingsSnapshot {
  const storageKey = localStorage.key(0);
  if (storageKey === null) {
    throw new Error("Expected settings state to be persisted");
  }
  const serializedState = localStorage.getItem(storageKey);
  if (serializedState === null) {
    throw new Error("Expected persisted settings state to be readable");
  }

  return { storageKey, serializedState };
}

function restorePersistedSettingsSnapshot(
  snapshot: PersistedSettingsSnapshot,
): void {
  localStorage.setItem(snapshot.storageKey, snapshot.serializedState);
}

function confirmCurrentCandidates(): void {
  const state = useSettingsStore.getState();
  state.confirmDurationSettingsCandidate(
    createDurationSettingsCandidate(state),
  );
  state.confirmAutomationSettingsCandidate(
    createAutomationSettingsCandidate(state),
  );
}

function resetThroughAction(resetAction: ResetAction): void {
  const state = useSettingsStore.getState();
  if (resetAction === "resetToDefaults") {
    state.resetToDefaults();
    return;
  }
  state.reset();
}

function createWeekdayMutationCases(): CandidateMutationCase[] {
  return WEEKDAY_KEYS.flatMap((weekday) => [
    {
      label: `${weekday} enabled`,
      group: "automation" as const,
      mutate: (): void => {
        useSettingsStore.getState().setDayAutoRunEnabled(weekday, true);
      },
    },
    {
      label: `${weekday} start time`,
      group: "automation" as const,
      mutate: (): void => {
        useSettingsStore.getState().setDayStartTime(weekday, "10:00");
      },
    },
    {
      label: `${weekday} end time`,
      group: "automation" as const,
      mutate: (): void => {
        useSettingsStore.getState().setDayEndTime(weekday, "17:00");
      },
    },
  ]);
}

const candidateMutationCases: CandidateMutationCase[] = [
  {
    label: "work duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore.getState().setWorkDuration(30);
    },
  },
  {
    label: "short break duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore.getState().setShortBreakDuration(10);
    },
  },
  {
    label: "long break duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore.getState().setLongBreakDuration(20);
    },
  },
  {
    label: "long break interval",
    group: "duration",
    mutate: (): void => {
      useSettingsStore.getState().setLongBreakInterval(6);
    },
  },
  {
    label: "auto-run feature flag",
    group: "automation",
    mutate: (): void => {
      useSettingsStore.getState().setAutoRunFeatureEnabled(true);
    },
  },
  {
    label: "post-break transition policy",
    group: "automation",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setPostBreakTransitionPolicy("start_next_work_automatically");
    },
  },
  ...createWeekdayMutationCases(),
];

const unchangedCandidateWriteCases: CandidateMutationCase[] = [
  {
    label: "work duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setWorkDuration(DEFAULT_SETTINGS.workDuration);
    },
  },
  {
    label: "short break duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setShortBreakDuration(DEFAULT_SETTINGS.shortBreakDuration);
    },
  },
  {
    label: "long break duration",
    group: "duration",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setLongBreakDuration(DEFAULT_SETTINGS.longBreakDuration);
    },
  },
  {
    label: "long break interval",
    group: "duration",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setLongBreakInterval(DEFAULT_SETTINGS.longBreakInterval);
    },
  },
  {
    label: "auto-run feature flag",
    group: "automation",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setAutoRunFeatureEnabled(DEFAULT_SETTINGS.autoRunFeatureEnabled);
    },
  },
  {
    label: "post-break transition policy",
    group: "automation",
    mutate: (): void => {
      useSettingsStore
        .getState()
        .setPostBreakTransitionPolicy(
          DEFAULT_SETTINGS.postBreakTransitionPolicy,
        );
    },
  },
  {
    label: "weekday enabled value",
    group: "automation",
    mutate: (): void => {
      useSettingsStore.getState().setDayAutoRunEnabled("mon", false);
    },
  },
  {
    label: "weekday start time",
    group: "automation",
    mutate: (): void => {
      useSettingsStore.getState().setDayStartTime("mon", "09:00");
    },
  },
  {
    label: "weekday end time",
    group: "automation",
    mutate: (): void => {
      useSettingsStore.getState().setDayEndTime("mon", "18:00");
    },
  },
];

describe("settings application state", () => {
  beforeEach(() => {
    localStorage.clear();
    resetSettingsStoreForTest();
  });

  it("starts both setting groups pending", () => {
    const state = useSettingsStore.getInitialState();

    expect(state.durationSettingsApplicationStatus).toBe("pending");
    expect(state.automationSettingsApplicationStatus).toBe("pending");
  });

  it.each(candidateMutationCases)(
    "moves only the $group group to pending when $label changes",
    ({ group, mutate }) => {
      confirmCurrentCandidates();

      mutate();

      const state = useSettingsStore.getState();
      expect(state.durationSettingsApplicationStatus).toBe(
        group === "duration" ? "pending" : "confirmed",
      );
      expect(state.automationSettingsApplicationStatus).toBe(
        group === "automation" ? "pending" : "confirmed",
      );
    },
  );

  it("treats a changed duration as a new pending candidate after rejection", () => {
    const initialState = useSettingsStore.getState();
    initialState.markDurationSettingsCandidateUnconfirmed(
      createDurationSettingsCandidate(initialState),
    );
    initialState.markAutomationSettingsCandidateUnconfirmed(
      createAutomationSettingsCandidate(initialState),
    );

    useSettingsStore.getState().setWorkDuration(30);

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("pending");
    expect(state.automationSettingsApplicationStatus).toBe("unconfirmed");
  });

  it("treats changed automation as a new pending candidate after rejection", () => {
    const initialState = useSettingsStore.getState();
    initialState.markDurationSettingsCandidateUnconfirmed(
      createDurationSettingsCandidate(initialState),
    );
    initialState.markAutomationSettingsCandidateUnconfirmed(
      createAutomationSettingsCandidate(initialState),
    );

    useSettingsStore.getState().setAutoRunFeatureEnabled(true);

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("unconfirmed");
    expect(state.automationSettingsApplicationStatus).toBe("pending");
  });

  it.each(unchangedCandidateWriteCases)(
    "keeps both groups confirmed when $label is written unchanged",
    ({ mutate }) => {
      confirmCurrentCandidates();

      mutate();

      const state = useSettingsStore.getState();
      expect(state.durationSettingsApplicationStatus).toBe("confirmed");
      expect(state.automationSettingsApplicationStatus).toBe("confirmed");
    },
  );

  it("marks both changed candidates pending after a bulk update", () => {
    confirmCurrentCandidates();
    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    weeklyAutoRunConfig.tue.enabled = true;

    useSettingsStore.getState().updateSettings({
      workDuration: 40,
      autoRunFeatureEnabled: true,
      weeklyAutoRunConfig,
    });

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("pending");
    expect(state.automationSettingsApplicationStatus).toBe("pending");
  });

  it("keeps confirmed status when a bulk update has no effective change", () => {
    useSettingsStore.getState().setWorkDuration(DEFAULT_SETTINGS.workDuration);
    confirmCurrentCandidates();

    useSettingsStore.getState().updateSettings({
      workDuration: DEFAULT_SETTINGS.workDuration,
      shortBreakDuration: DEFAULT_SETTINGS.shortBreakDuration,
      longBreakDuration: DEFAULT_SETTINGS.longBreakDuration,
      longBreakInterval: DEFAULT_SETTINGS.longBreakInterval,
      autoRunFeatureEnabled: DEFAULT_SETTINGS.autoRunFeatureEnabled,
      postBreakTransitionPolicy: DEFAULT_SETTINGS.postBreakTransitionPolicy,
      weeklyAutoRunConfig: createWeeklyAutoRunConfigAllDaysDisabled(),
    });

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("confirmed");
    expect(state.automationSettingsApplicationStatus).toBe("confirmed");
  });

  it("uses the validated candidate when deciding whether a bulk write changed", () => {
    useSettingsStore.getState().setWorkDuration(DEFAULT_SETTINGS.workDuration);
    useSettingsStore.getState().setWorkDuration(120);
    confirmCurrentCandidates();

    useSettingsStore.getState().updateSettings({ workDuration: 500 });

    expect(useSettingsStore.getState().workDuration).toBe(120);
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
  });

  it.each([
    {
      label: "notification volume",
      mutate: (): void => {
        useSettingsStore.getState().setNotificationSoundVolumePercent(75);
      },
    },
    {
      label: "notification mute state",
      mutate: (): void => {
        useSettingsStore.getState().toggleNotificationSoundMuted();
      },
    },
    {
      label: "bulk notification values",
      mutate: (): void => {
        useSettingsStore.getState().updateSettings({
          notificationSoundVolumePercent: 75,
          notificationSoundMuted: true,
        });
      },
    },
  ])("keeps both group statuses when changing $label", ({ mutate }) => {
    const initialState = useSettingsStore.getState();
    initialState.markDurationSettingsCandidateUnconfirmed(
      createDurationSettingsCandidate(initialState),
    );
    initialState.confirmAutomationSettingsCandidate(
      createAutomationSettingsCandidate(initialState),
    );

    mutate();

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("unconfirmed");
    expect(state.automationSettingsApplicationStatus).toBe("confirmed");
  });

  it("confirms only exact current candidates", () => {
    const initialState = useSettingsStore.getState();

    initialState.confirmDurationSettingsCandidate(
      createDurationSettingsCandidate(initialState),
    );

    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("pending");

    const durationConfirmedState = useSettingsStore.getState();
    durationConfirmedState.confirmAutomationSettingsCandidate(
      createAutomationSettingsCandidate(durationConfirmedState),
    );

    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("confirmed");
  });

  it("marks only exact current candidates unconfirmed", () => {
    const initialState = useSettingsStore.getState();

    initialState.markDurationSettingsCandidateUnconfirmed(
      createDurationSettingsCandidate(initialState),
    );

    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "unconfirmed",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("pending");

    const durationRejectedState = useSettingsStore.getState();
    durationRejectedState.markAutomationSettingsCandidateUnconfirmed(
      createAutomationSettingsCandidate(durationRejectedState),
    );

    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("unconfirmed");
  });

  it.each(["success", "failure"] as const)(
    "ignores stale duration synchronization %s",
    (result) => {
      const capturedCandidate = createDurationSettingsCandidate(
        useSettingsStore.getState(),
      );
      useSettingsStore.getState().setWorkDuration(35);

      const state = useSettingsStore.getState();
      if (result === "success") {
        state.confirmDurationSettingsCandidate(capturedCandidate);
      } else {
        state.markDurationSettingsCandidateUnconfirmed(capturedCandidate);
      }

      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("pending");
    },
  );

  it.each(["success", "failure"] as const)(
    "ignores stale automation synchronization %s",
    (result) => {
      const capturedCandidate = createAutomationSettingsCandidate(
        useSettingsStore.getState(),
      );
      useSettingsStore.getState().setDayStartTime("sun", "10:00");

      const state = useSettingsStore.getState();
      if (result === "success") {
        state.confirmAutomationSettingsCandidate(capturedCandidate);
      } else {
        state.markAutomationSettingsCandidateUnconfirmed(capturedCandidate);
      }

      expect(
        useSettingsStore.getState().automationSettingsApplicationStatus,
      ).toBe("pending");
    },
  );

  it("preserves confirmed duration status when readback matches", () => {
    confirmCurrentCandidates();
    const currentCandidate = createDurationSettingsCandidate(
      useSettingsStore.getState(),
    );

    useSettingsStore
      .getState()
      .reconcileDurationSettingsCandidateReadback(currentCandidate, true);

    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
  });

  it("invalidates only a current confirmed duration candidate on mismatch", () => {
    confirmCurrentCandidates();
    const currentCandidate = createDurationSettingsCandidate(
      useSettingsStore.getState(),
    );

    useSettingsStore
      .getState()
      .reconcileDurationSettingsCandidateReadback(currentCandidate, false);

    const state = useSettingsStore.getState();
    expect(state.durationSettingsApplicationStatus).toBe("pending");
    expect(state.automationSettingsApplicationStatus).toBe("confirmed");
  });

  it("ignores a stale readback mismatch for a newer confirmed candidate", () => {
    const staleCandidate = createDurationSettingsCandidate(
      useSettingsStore.getState(),
    );
    useSettingsStore.getState().setWorkDuration(35);
    const currentState = useSettingsStore.getState();
    currentState.confirmDurationSettingsCandidate(
      createDurationSettingsCandidate(currentState),
    );

    useSettingsStore
      .getState()
      .reconcileDurationSettingsCandidateReadback(staleCandidate, false);

    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
  });

  it.each([
    { status: "pending" as const, readbackMatchesCandidate: true },
    { status: "pending" as const, readbackMatchesCandidate: false },
    { status: "unconfirmed" as const, readbackMatchesCandidate: true },
    { status: "unconfirmed" as const, readbackMatchesCandidate: false },
  ])(
    "does not promote or demote $status duration after readback",
    ({ status, readbackMatchesCandidate }) => {
      const candidate = createDurationSettingsCandidate(
        useSettingsStore.getState(),
      );
      if (status === "unconfirmed") {
        useSettingsStore
          .getState()
          .markDurationSettingsCandidateUnconfirmed(candidate);
      }

      useSettingsStore
        .getState()
        .reconcileDurationSettingsCandidateReadback(
          candidate,
          readbackMatchesCandidate,
        );

      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe(status);
    },
  );

  it.each([
    {
      resetAction: "resetToDefaults",
      changedGroup: "duration" as const,
      prepare: (): void => {
        useSettingsStore.getState().setWorkDuration(40);
      },
    },
    {
      resetAction: "resetToDefaults",
      changedGroup: "automation" as const,
      prepare: (): void => {
        useSettingsStore.getState().setAutoRunFeatureEnabled(true);
      },
    },
    {
      resetAction: "reset",
      changedGroup: "duration" as const,
      prepare: (): void => {
        useSettingsStore.getState().setWorkDuration(40);
      },
    },
    {
      resetAction: "reset",
      changedGroup: "automation" as const,
      prepare: (): void => {
        useSettingsStore.getState().setAutoRunFeatureEnabled(true);
      },
    },
  ] satisfies ResetMutationCase[])(
    "$resetAction marks only a changed $changedGroup candidate pending",
    ({ resetAction, changedGroup, prepare }) => {
      prepare();
      confirmCurrentCandidates();

      resetThroughAction(resetAction);

      const state = useSettingsStore.getState();
      expect(state.durationSettingsApplicationStatus).toBe(
        changedGroup === "duration" ? "pending" : "confirmed",
      );
      expect(state.automationSettingsApplicationStatus).toBe(
        changedGroup === "automation" ? "pending" : "confirmed",
      );
    },
  );

  it.each(["resetToDefaults", "reset"] as const)(
    "%s preserves statuses for unchanged candidates",
    (resetAction) => {
      const initialState = useSettingsStore.getState();
      initialState.confirmDurationSettingsCandidate(
        createDurationSettingsCandidate(initialState),
      );
      initialState.markAutomationSettingsCandidateUnconfirmed(
        createAutomationSettingsCandidate(initialState),
      );

      resetThroughAction(resetAction);

      const state = useSettingsStore.getState();
      expect(state.durationSettingsApplicationStatus).toBe("confirmed");
      expect(state.automationSettingsApplicationStatus).toBe("unconfirmed");
    },
  );

  it("discards a persisted snapshot with a malformed weekly schedule", async () => {
    useSettingsStore.setState({
      workDuration: 40,
      autoRunFeatureEnabled: true,
      weeklyAutoRunConfig: null as unknown as WeeklyAutoRunConfig,
      durationSettingsApplicationStatus: "confirmed",
      automationSettingsApplicationStatus: "confirmed",
    });
    const malformedSnapshot = capturePersistedSettingsSnapshot();

    resetSettingsStoreForTest();
    restorePersistedSettingsSnapshot(malformedSnapshot);
    await useSettingsStore.persist.rehydrate();

    const state = useSettingsStore.getState();
    expect(state.workDuration).toBe(DEFAULT_SETTINGS.workDuration);
    expect(state.autoRunFeatureEnabled).toBe(
      DEFAULT_SETTINGS.autoRunFeatureEnabled,
    );
    expect(state.weeklyAutoRunConfig).toEqual(
      DEFAULT_SETTINGS.weeklyAutoRunConfig,
    );
    expect(state.durationSettingsApplicationStatus).toBe("pending");
    expect(state.automationSettingsApplicationStatus).toBe("pending");
    expect(() => {
      state.setAutoRunFeatureEnabled(true);
    }).not.toThrow();
  });

  it.each([
    {
      durationStatus: "pending" as const,
      automationStatus: "unconfirmed" as const,
    },
    {
      durationStatus: "unconfirmed" as const,
      automationStatus: "pending" as const,
    },
    {
      durationStatus: "confirmed" as const,
      automationStatus: "confirmed" as const,
    },
  ])(
    "restores $durationStatus duration and $automationStatus automation through rehydration",
    async ({ durationStatus, automationStatus }) => {
      useSettingsStore.getState().setWorkDuration(40);
      useSettingsStore.getState().setAutoRunFeatureEnabled(true);

      let state = useSettingsStore.getState();
      if (durationStatus === "unconfirmed") {
        state.markDurationSettingsCandidateUnconfirmed(
          createDurationSettingsCandidate(state),
        );
      }
      if (durationStatus === "confirmed") {
        state.confirmDurationSettingsCandidate(
          createDurationSettingsCandidate(state),
        );
      }
      state = useSettingsStore.getState();
      if (automationStatus === "unconfirmed") {
        state.markAutomationSettingsCandidateUnconfirmed(
          createAutomationSettingsCandidate(state),
        );
      }
      if (automationStatus === "confirmed") {
        state.confirmAutomationSettingsCandidate(
          createAutomationSettingsCandidate(state),
        );
      }

      const expectedDurationCandidate = createDurationSettingsCandidate(
        useSettingsStore.getState(),
      );
      const expectedAutomationCandidate = createAutomationSettingsCandidate(
        useSettingsStore.getState(),
      );
      const persistedSnapshot = capturePersistedSettingsSnapshot();

      useSettingsStore.setState({
        workDuration: DEFAULT_SETTINGS.workDuration,
        autoRunFeatureEnabled: DEFAULT_SETTINGS.autoRunFeatureEnabled,
        durationSettingsApplicationStatus:
          durationStatus === "confirmed" ? "pending" : "confirmed",
        automationSettingsApplicationStatus:
          automationStatus === "confirmed" ? "pending" : "confirmed",
      });
      restorePersistedSettingsSnapshot(persistedSnapshot);

      await useSettingsStore.persist.rehydrate();

      state = useSettingsStore.getState();
      expect(createDurationSettingsCandidate(state)).toEqual(
        expectedDurationCandidate,
      );
      expect(createAutomationSettingsCandidate(state)).toEqual(
        expectedAutomationCandidate,
      );
      expect(state.durationSettingsApplicationStatus).toBe(durationStatus);
      expect(state.automationSettingsApplicationStatus).toBe(automationStatus);
    },
  );
});
