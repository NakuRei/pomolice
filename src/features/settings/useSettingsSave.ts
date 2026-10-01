import { saveSettings } from "./commands";
import type { SettingsValidationIssue } from "@/ipc/generated";
import { useState } from "react";
import type { SettingsChanges } from "./settings";

export interface SettingsSave {
  /** What the backend rejected on the last save, until the user edits it. */
  issues: SettingsValidationIssue[];
  isSaving: boolean;
  save: (changes: SettingsChanges) => Promise<void>;
  /** Drops the issues that an edit has settled. */
  settleIssues: (
    isSettled: (issue: SettingsValidationIssue) => boolean,
  ) => void;
}

/**
 * How a settings page saves: a successful save calls `onSaved`, a rejection
 * keeps its issues for the page to show next to the inputs, and any other
 * failure is reported by the store.
 */
export function useSettingsSave(onSaved: () => void): SettingsSave {
  const [issues, setIssues] = useState<SettingsValidationIssue[]>([]);
  const [isSaving, setIsSaving] = useState(false);

  const save = async (changes: SettingsChanges): Promise<void> => {
    setIsSaving(true);
    const result = await saveSettings(changes);
    setIsSaving(false);
    if (result.status === "accepted") {
      onSaved();
      return;
    }
    if (
      result.status === "rejected" &&
      result.error.code === "invalid_settings"
    ) {
      setIssues(result.error.issues);
    }
  };

  const settleIssues = (
    isSettled: (issue: SettingsValidationIssue) => boolean,
  ): void => {
    setIssues((previousIssues) =>
      previousIssues.filter((issue) => !isSettled(issue)),
    );
  };

  return { issues, isSaving, save, settleIssues };
}
