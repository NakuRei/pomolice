import {
  commands,
  type CommandResponse,
  type TimerSnapshot,
} from "@/ipc/generated";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "./operationStore";
import type { OperationResult, Operation } from "./operation";
import { settingsChangesToPayload } from "./settings";

function sendOperation(
  request: Operation,
): Promise<CommandResponse<TimerSnapshot>> {
  switch (request.kind) {
    case "start_work":
      return commands.startWork();
    case "emergency_stop":
      return commands.emergencyStop();
    case "resolve_post_break_decision":
      return commands.resolvePostBreakDecision(request.action);
    case "set_away_during_break":
      return commands.setAwayDuringBreak(request.away);
    case "update_settings":
      return commands.updateSettings(settingsChangesToPayload(request.changes));
    case "resolve_boot_auto_start_prompt":
      return commands.resolveBootAutoStartPrompt(request.startNow);
    case "retry_failed_timer_effects":
      return commands.retryFailedTimerEffects();
    default: {
      const unreachable: never = request;
      throw new Error(`Unknown operation: ${String(unreachable)}`);
    }
  }
}

async function recordOperation(request: Operation): Promise<OperationResult> {
  const id = useOperationStore.getState().begin(request);
  const result = await receiveCommandResponse(sendOperation(request));
  if (result.status === "accepted") {
    useBackendObservationStore.getState().observeSnapshot(result.value);
  }
  useOperationStore
    .getState()
    .complete(
      request.kind,
      id,
      result.status === "accepted" ? { status: "accepted" } : result,
    );
  return result;
}

let pendingEffectRetry: Promise<OperationResult> | null = null;

export function executeOperation(request: Operation): Promise<OperationResult> {
  if (request.kind === "retry_failed_timer_effects") {
    pendingEffectRetry ??= recordOperation(request).finally(() => {
      pendingEffectRetry = null;
    });
    return pendingEffectRetry;
  }
  return recordOperation(structuredClone(request));
}
