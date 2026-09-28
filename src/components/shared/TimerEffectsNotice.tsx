import { useTimerEffectsStore } from "@/store/timerEffectsStore";
import { StatusNotice } from "./StatusNotice";
import { Button } from "./Button";

const LOCK_FAILURE_MESSAGES = {
  enter: "画面のロックに失敗しました。",
  release: "画面のロック解除に失敗しました。",
};

export function TimerEffectsNotice(): React.ReactElement | null {
  const effects = useTimerEffectsStore((state) => state.effects);
  const retrying = useTimerEffectsStore((state) => state.retrying);
  const retryError = useTimerEffectsStore((state) => state.retryError);
  const retry = useTimerEffectsStore((state) => state.retry);
  if (effects === null) return null;
  const lock = effects.break_lock.last_attempt;
  const lockFailed = lock?.status === "failed";
  const saveFailed = effects.last_timer_save === "failed";
  if (!lockFailed && !saveFailed) return null;

  return (
    <StatusNotice
      tone="danger"
      role="alert"
      actions={
        <Button
          appearance="secondary"
          disabled={retrying}
          onClick={() => {
            void retry();
          }}
        >
          {retrying ? "再試行中…" : "再試行"}
        </Button>
      }
    >
      {lockFailed && <p>{LOCK_FAILURE_MESSAGES[lock.action]}</p>}
      {saveFailed && (
        <p>タイマーの状態の保存に失敗しました。操作は反映されています。</p>
      )}
      {retryError !== null && <p>{retryError}</p>}
    </StatusNotice>
  );
}
