import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
import { useTimerObservationStore } from "@/store/timerObservationStore";
import { useTimerOperationStore } from "./operationStore";
import type { TimerCommandResult, TimerOperation } from "./operation";

export async function executeTimerCommand(
  request: TimerOperation,
  send: () => Promise<CommandResponse<TimerSnapshot>>,
): Promise<TimerCommandResult> {
  const id = useTimerOperationStore.getState().begin(request);
  const result = await receiveCommandResponse(send());
  if (result.status === "accepted")
    useTimerObservationStore.getState().observeEffects(result.value.effects);
  useTimerOperationStore
    .getState()
    .complete(
      request.kind,
      id,
      result.status === "accepted" ? { status: "accepted" } : result,
    );
  return result;
}
