/**
 * Window Control Service
 *
 * Frontend wrapper for Tauri timer state management commands.
 * All state is managed by the backend - these are convenience wrappers.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  PostBreakDecisionAction,
  PostBreakTransitionPolicy,
} from "../types/sessionTransitionPolicy";

/**
 * Start a work session with settings
 *
 * @param settings - Timer settings
 */
export async function startWork(settings: {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
  post_break_transition_policy: PostBreakTransitionPolicy;
}): Promise<void> {
  await invoke("start_work", {
    workDurationSecs: settings.workDurationSecs,
    shortBreakDurationSecs: settings.shortBreakDurationSecs,
    longBreakDurationSecs: settings.longBreakDurationSecs,
    longBreakInterval: settings.longBreakInterval,
    postBreakTransitionPolicy: settings.post_break_transition_policy,
  });
}

/**
 * Resolve manual post-break decision.
 */
export async function resolvePostBreakDecision(
  action: PostBreakDecisionAction
): Promise<void> {
  await invoke("resolve_post_break_decision", { action });
}

/**
 * Emergency stop - reset all state
 */
export async function emergencyStop(): Promise<void> {
  await invoke("emergency_stop");
}

/**
 * Sync settings to backend
 */
export async function syncSettings(settings: {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
  post_break_transition_policy: PostBreakTransitionPolicy;
  autoRunFeatureEnabled: boolean;
  weeklyAutoRunConfig: Record<string, unknown>;
}): Promise<void> {
  await invoke("sync_timer_duration_settings", {
    workDurationSecs: settings.workDurationSecs,
    shortBreakDurationSecs: settings.shortBreakDurationSecs,
    longBreakDurationSecs: settings.longBreakDurationSecs,
    longBreakInterval: settings.longBreakInterval,
  });
  await invoke("sync_automation_policies", {
    postBreakTransitionPolicy: settings.post_break_transition_policy,
    autoRunFeatureEnabled: settings.autoRunFeatureEnabled,
    weeklyAutoRunConfig: settings.weeklyAutoRunConfig,
  });
}

/**
 * Get current timer status
 *
 * @returns "idle" | "working" | "on_break"
 */
export async function getTimerStatus(): Promise<string> {
  return await invoke("get_timer_status");
}

/**
 * Record a skip attempt
 *
 * @returns Number of skip attempts in current session
 */
export async function recordSkipAttempt(): Promise<number> {
  return await invoke("record_skip_attempt");
}

