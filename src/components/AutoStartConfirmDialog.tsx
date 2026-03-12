import { useEffect, useRef } from "react";
import { useTimerStore } from "../store/timerStore";
import type { WeekdayKey } from "../types/weeklyAutoRun";
import { DialogActionRow } from "./shared/DialogActionRow";
import { DialogOverlay } from "./shared/DialogOverlay";
import { DialogPanel } from "./shared/DialogPanel";
import { InlineError } from "./shared/InlineError";

const WEEKDAY_LABELS: Record<WeekdayKey, string> = {
  mon: "月曜日",
  tue: "火曜日",
  wed: "水曜日",
  thu: "木曜日",
  fri: "金曜日",
  sat: "土曜日",
  sun: "日曜日",
};

export function AutoStartConfirmDialog() {
  const shouldPromptStartOnBoot = useTimerStore((state) => state.shouldPromptStartOnBoot);
  const activeWindowSummary = useTimerStore((state) => state.activeWindowSummary);
  const resolveBootAutoStartPrompt = useTimerStore((state) => state.resolveBootAutoStartPrompt);
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

  const weekdayLabel = WEEKDAY_LABELS[activeWindowSummary.weekday_key];
  const crossesMidnightSuffix = activeWindowSummary.crosses_midnight ? "（日跨ぎ）" : "";

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
          現在は {weekdayLabel} {activeWindowSummary.start_time} - {activeWindowSummary.end_time}
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
          <button
            ref={startButtonRef}
            onClick={() => {
              resolvePrompt(true);
            }}
            className="flex-1 rounded-lg bg-blue-600 px-6 py-3 text-white transition-colors hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
            data-testid="auto-start-confirm-start"
          >
            開始する
          </button>
          <button
            onClick={() => {
              resolvePrompt(false);
            }}
            className="flex-1 rounded-lg bg-gray-200 px-6 py-3 text-gray-700 transition-colors hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-400"
            data-testid="auto-start-confirm-later"
          >
            あとで開始する
          </button>
        </DialogActionRow>
      </DialogPanel>
    </DialogOverlay>
  );
}
