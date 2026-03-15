import { useEffect, useRef } from "react";
import { WEEKDAY_LABELS_BY_KEY } from "../constants/weekdayLabels";
import { useTimerStore } from "../store/timerStore";
import { Button } from "./shared/Button";
import { DialogActionRow } from "./shared/DialogActionRow";
import { DialogOverlay } from "./shared/DialogOverlay";
import { DialogPanel } from "./shared/DialogPanel";
import { InlineError } from "./shared/InlineError";

export function AutoStartConfirmDialog() {
  const shouldPromptStartOnBoot = useTimerStore(
    (state) => state.shouldPromptStartOnBoot,
  );
  const activeWindowSummary = useTimerStore(
    (state) => state.activeWindowSummary,
  );
  const resolveBootAutoStartPrompt = useTimerStore(
    (state) => state.resolveBootAutoStartPrompt,
  );
  const startButtonRef = useRef<HTMLButtonElement>(null);

  const isVisible = shouldPromptStartOnBoot && activeWindowSummary !== null;

  const resolvePrompt = (startNow: boolean) => {
    resolveBootAutoStartPrompt(startNow).catch(() => undefined);
  };

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

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      resolvePrompt(false);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isVisible, resolveBootAutoStartPrompt]);

  if (!isVisible || activeWindowSummary === null) {
    return null;
  }

  const weekdayLabel =
    WEEKDAY_LABELS_BY_KEY[activeWindowSummary.weekday_key].full;
  const crossesMidnightSuffix = activeWindowSummary.crosses_midnight
    ? "（日跨ぎ）"
    : "";

  return (
    <DialogOverlay testId="auto-start-confirm-dialog-overlay">
      <DialogPanel
        labelledBy="auto-start-confirm-dialog-title"
        testId="auto-start-confirm-dialog"
      >
        <h2
          id="auto-start-confirm-dialog-title"
          className="mb-4 text-2xl font-bold text-gray-900"
        >
          作業を開始しますか？
        </h2>
        <p className="mb-2 text-gray-700">
          現在は {weekdayLabel} {activeWindowSummary.start_time} -{" "}
          {activeWindowSummary.end_time}
          {crossesMidnightSuffix} の自動実行枠です。
        </p>
        <p className="mb-4 text-gray-700">
          開始すると終了時刻連動でセッション区切り停止されます。
        </p>
        <div className="mb-4">
          <InlineError
            errorId="resolve_boot_auto_start_prompt"
            onRetry={() => {
              resolvePrompt(true);
            }}
          />
        </div>
        <DialogActionRow>
          <Button
            ref={startButtonRef}
            appearance="primary"
            size="md"
            onClick={() => {
              resolvePrompt(true);
            }}
            className="flex-1"
            testId="auto-start-confirm-start"
          >
            開始する
          </Button>
          <Button
            appearance="secondary"
            size="md"
            onClick={() => {
              resolvePrompt(false);
            }}
            className="flex-1"
            testId="auto-start-confirm-later"
          >
            あとで開始する
          </Button>
        </DialogActionRow>
      </DialogPanel>
    </DialogOverlay>
  );
}
