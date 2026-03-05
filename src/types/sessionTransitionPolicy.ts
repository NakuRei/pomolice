/**
 * Policy for deciding what happens when a break session finishes.
 *
 * Usage example:
 * ```ts
 * const policy: PostBreakTransitionPolicy = "require_manual_continue";
 * ```
 */
export type PostBreakTransitionPolicy =
  | "require_manual_continue"
  | "start_next_work_automatically";

/**
 * Action from the manual post-break decision UI.
 *
 * Usage example:
 * ```ts
 * const action: PostBreakDecisionAction = "continue";
 * ```
 */
export type PostBreakDecisionAction = "continue" | "stop_cycle";

/**
 * Reason emitted at a session boundary transition.
 *
 * Usage example:
 * ```ts
 * const reason: SessionTransitionReason = "break_waiting_decision";
 * ```
 */
export type SessionTransitionReason =
  | "work_to_break"
  | "break_waiting_decision"
  | "break_auto_started"
  | "scheduled_stop_executed";

/**
 * Unified session-boundary transition contract.
 * This contract is consumed by frontend state updates and notification routing.
 *
 * Usage example:
 * ```ts
 * const outcome: SessionTransitionOutcome = {
 *   transition_reason: "break_auto_started",
 *   completed_session_type: "short_break",
 *   next_status: "working",
 *   next_session_type: "work",
 *   requires_manual_continue_decision: false,
 * };
 * ```
 */
export interface SessionTransitionOutcome {
  transition_reason: SessionTransitionReason;
  completed_session_type: "work" | "short_break" | "long_break";
  next_status: "idle" | "working" | "on_break";
  next_session_type: "work" | "short_break" | "long_break";
  requires_manual_continue_decision: boolean;
}

/**
 * Frontend default policy value.
 * The default keeps behavior predictable by requiring manual confirmation.
 */
export const DEFAULT_POST_BREAK_TRANSITION_POLICY: PostBreakTransitionPolicy =
  "require_manual_continue";
