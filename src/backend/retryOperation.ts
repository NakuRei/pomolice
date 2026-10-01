import { useBackendObservationStore } from "@/backend/observationStore";
import type { OperationRecord } from "./operation";
import { useOperationStore } from "./operationStore";
import { classifyCommandFailure } from "@/errors/classifyBackendError";
import { executeOperation } from "./executeOperation";

export function canRetryOperation(
  record: OperationRecord,
  hasBootOffer: boolean,
): boolean {
  if (record.outcome.status !== "rejected") return false;
  return (
    classifyCommandFailure(record.outcome).canRetry &&
    (record.request.kind !== "resolve_boot_auto_start_prompt" || hasBootOffer)
  );
}

export async function retryRejectedOperation(
  record: OperationRecord,
): Promise<void> {
  const current = useOperationStore.getState().operations[record.request.kind];
  if (
    current?.id !== record.id ||
    !canRetryOperation(
      current,
      Boolean(useBackendObservationStore.getState().bootAutoStartOffer),
    )
  )
    return;
  await executeOperation(current.request);
}
