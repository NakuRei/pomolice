import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "@/backend/operationStore";
import { executeOperation } from "@/backend/executeOperation";
import type { TimerEffectsState } from "@/ipc/generated";
import { StatusNotice } from "@/components/shared/StatusNotice";
import { Button } from "@/components/shared/Button";

const LOCK_FAILURE_MESSAGES = {
  enter: "画面のロックに失敗しました。",
  release: "画面のロック解除に失敗しました。",
};

function effectFailureMessages(effects: TimerEffectsState | null): string[] {
  if (effects === null) return [];
  const messages: string[] = [];
  const lock = effects.break_lock.last_attempt;
  if (lock?.status === "failed")
    messages.push(LOCK_FAILURE_MESSAGES[lock.action]);
  if (effects.last_timer_save === "failed")
    messages.push(
      "タイマーの状態の保存に失敗しました。操作は反映されています。",
    );
  return messages;
}

export function TimerEffectsNotice(): React.ReactElement | null {
  const effects = useBackendObservationStore((state) => state.effects);
  const retrying = useOperationStore(
    (state) =>
      state.operations.retry_failed_timer_effects?.outcome.status === "pending",
  );
  const messages = effectFailureMessages(effects);
  if (messages.length === 0) return null;

  return (
    <StatusNotice
      tone="danger"
      role="alert"
      actions={
        <Button
          appearance="secondary"
          disabled={retrying}
          onClick={() => {
            void executeOperation({ kind: "retry_failed_timer_effects" });
          }}
        >
          {retrying ? "再試行中…" : "再試行"}
        </Button>
      }
    >
      {messages.map((message) => (
        <p key={message}>{message}</p>
      ))}
    </StatusNotice>
  );
}
