import { useEffect, useId, useRef } from "react";
import { WEEKDAY_LABELS_BY_KEY } from "../constants/weekdayLabels";
import { useTimerStore } from "../store/timerStore";
import { Button } from "./shared/Button";
import { DialogActionRow } from "./shared/DialogActionRow";
import { AppDialog } from "./shared/AppDialog";
import { InlineError } from "./shared/InlineError";

export function AutoStartConfirmDialog(): React.ReactElement | null {
  const promptWindow = useTimerStore(
    (state) => state.bootAutoStartOffer?.window,
  );
  const resolveBootAutoStartPrompt = useTimerStore(
    (state) => state.resolveBootAutoStartPrompt,
  );
  const titleId = useId();
  const startButtonRef = useRef<HTMLButtonElement>(null);

  const isVisible = Boolean(promptWindow);
  useEffect(() => {
    if (!isVisible) {
      return;
    }
    startButtonRef.current?.focus();
  }, [isVisible]);

  useEffect(() => {
    if (!isVisible) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      void resolveBootAutoStartPrompt(false).catch(() => undefined);
    };

    window.addEventListener("keydown", onKeyDown);
    return (): void => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isVisible, resolveBootAutoStartPrompt]);

  if (!promptWindow) {
    return null;
  }

  const weekdayLabel = WEEKDAY_LABELS_BY_KEY[promptWindow.weekday_key].full;
  const crossesMidnightSuffix = promptWindow.crosses_midnight
    ? "（日跨ぎ）"
    : "";

  return (
    <AppDialog
      overlayTestId="auto-start-confirm-dialog-overlay"
      labelledBy={titleId}
      testId="auto-start-confirm-dialog"
    >
      <h2 id={titleId} className="mb-4 text-2xl font-bold text-text">
        作業を開始しますか？
      </h2>
      <p className="mb-2 text-text-muted">
        現在は {weekdayLabel} {promptWindow.start_time} -{" "}
        {promptWindow.end_time}
        {crossesMidnightSuffix} の自動実行枠です。
      </p>
      <p className="mb-4 text-text-muted">
        開始すると終了時刻連動でセッション区切り停止されます。
      </p>
      <div className="mb-4">
        <InlineError errorId="resolve_boot_auto_start_prompt" />
      </div>
      <DialogActionRow>
        <Button
          ref={startButtonRef}
          appearance="primary"
          onClick={() => {
            void resolveBootAutoStartPrompt(true).catch(() => undefined);
          }}
          className="flex-1"
          testId="auto-start-confirm-start"
        >
          開始する
        </Button>
        <Button
          appearance="secondary"
          onClick={() => {
            void resolveBootAutoStartPrompt(false).catch(() => undefined);
          }}
          className="flex-1"
          testId="auto-start-confirm-later"
        >
          あとで開始する
        </Button>
      </DialogActionRow>
    </AppDialog>
  );
}
