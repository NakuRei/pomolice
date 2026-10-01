import { useBackendObservationStore } from "@/backend/observationStore";
import { useEffect } from "react";
import { useOperationStore } from "@/backend/operationStore";
import { classifyCommandFailure } from "@/errors/classifyBackendError";
import {
  canRetryOperation,
  retryRejectedOperation,
} from "@/backend/retryOperation";
import { getOperationFailure, type OperationKind } from "@/backend/operation";
import type { CommandFailure } from "@/ipc/receiveCommandResponse";
import { WarningTriangleIcon } from "@/components/icons/AppIcons";
import { StatusNotice } from "./StatusNotice";

interface OperationNoticeProps {
  operation: OperationKind;
  onRetry?: (() => void | Promise<void>) | null;
}

function isFieldValidationFailure(failure: CommandFailure | null): boolean {
  return (
    failure?.status === "rejected" &&
    failure.error.code === "invalid_settings" &&
    failure.error.issues.length > 0
  );
}

export function OperationNotice({
  operation,
  onRetry,
}: OperationNoticeProps): React.ReactElement | null {
  const record = useOperationStore((state) => state.operations[operation]);
  const dismiss = useOperationStore((state) => state.dismiss);
  const hasBootOffer = useBackendObservationStore((state) =>
    Boolean(state.bootAutoStartOffer),
  );
  const failure = getOperationFailure(record?.outcome);
  const presentation =
    failure === null ? null : classifyCommandFailure(failure);
  const critical = operation === "emergency_stop";
  useEffect(() => {
    if (!record || record.dismissed || !failure || critical) return;
    const timeout = setTimeout(() => {
      dismiss(operation, record.id);
    }, 5000);
    return (): void => {
      clearTimeout(timeout);
    };
  }, [record, failure, critical, operation, dismiss]);
  if (!record || record.dismissed || !presentation) return null;
  if (isFieldValidationFailure(failure)) return null;
  const retryable = canRetryOperation(record, hasBootOffer) && onRetry !== null;
  return (
    <StatusNotice
      tone="danger"
      role="alert"
      testId={`inline-error-${operation}`}
      icon={
        <WarningTriangleIcon className="mt-0.5 h-4 w-4 shrink-0 text-danger-400" />
      }
      actions={
        retryable ? (
          <button
            onClick={() => {
              if (onRetry) void onRetry();
              else void retryRejectedOperation(record);
            }}
            className="shrink-0 font-medium underline hover:no-underline focus:ring-2 focus:ring-danger-500 focus:ring-offset-1 focus:outline-none"
            data-testid={`inline-error-retry-${operation}`}
          >
            再試行
          </button>
        ) : undefined
      }
    >
      {operation === "retry_failed_timer_effects"
        ? `直前の復旧操作：${presentation.userMessage}`
        : presentation.userMessage}
    </StatusNotice>
  );
}
