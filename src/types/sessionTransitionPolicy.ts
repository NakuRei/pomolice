import type { PostBreakTransitionPolicy } from "@/ipc/generated";

/**
 * Frontend default policy value.
 * The default keeps behavior predictable by requiring manual confirmation.
 */
export const DEFAULT_POST_BREAK_TRANSITION_POLICY: PostBreakTransitionPolicy =
  "require_manual_continue";
