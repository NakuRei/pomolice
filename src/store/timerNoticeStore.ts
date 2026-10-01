import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
export type InfoMessageId = "auto_run_stop_pending" | "auto_run_stopped";

export interface InfoMessage {
  id: InfoMessageId;
  message: string;
  ttlMs: number;
}

interface TimerNotices {
  infoMessage: InfoMessage | null;
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>;
  showAutoRunStopPendingInfo: () => void;
  showAutoRunStoppedInfo: () => void;
  clearInfoMessage: (expectedInfoMessageId?: InfoMessageId) => void;
}
const AUTO_RUN_STOP_PENDING_INFO_MESSAGE: InfoMessage = {
  id: "auto_run_stop_pending",
  message: "現在セッション終了後に停止予定",
  ttlMs: 5000,
};

const AUTO_RUN_STOPPED_INFO_MESSAGE: InfoMessage = {
  id: "auto_run_stopped",
  message: "自動停止しました",
  ttlMs: 3500,
};

function shouldSuppressInfoMessageByTtl(
  nowEpochMs: number,
  infoMessage: InfoMessage,
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>,
): boolean {
  const lastShownAtEpochMs = infoMessageShownAtById[infoMessage.id];
  return (
    lastShownAtEpochMs !== undefined &&
    nowEpochMs - lastShownAtEpochMs < infoMessage.ttlMs
  );
}

export const useTimerNoticeStore = create<TimerNotices>()(
  immer((set) => ({
    infoMessage: null,
    infoMessageShownAtById: {},
    showAutoRunStopPendingInfo: (): void => {
      set((state) => {
        const nowEpochMs = Date.now();
        if (
          shouldSuppressInfoMessageByTtl(
            nowEpochMs,
            AUTO_RUN_STOP_PENDING_INFO_MESSAGE,
            state.infoMessageShownAtById,
          )
        ) {
          return;
        }
        state.infoMessage = AUTO_RUN_STOP_PENDING_INFO_MESSAGE;
        state.infoMessageShownAtById.auto_run_stop_pending = nowEpochMs;
      });
    },

    showAutoRunStoppedInfo: (): void => {
      set((state) => {
        const nowEpochMs = Date.now();
        if (
          shouldSuppressInfoMessageByTtl(
            nowEpochMs,
            AUTO_RUN_STOPPED_INFO_MESSAGE,
            state.infoMessageShownAtById,
          )
        ) {
          return;
        }
        state.infoMessage = AUTO_RUN_STOPPED_INFO_MESSAGE;
        state.infoMessageShownAtById.auto_run_stopped = nowEpochMs;
      });
    },

    clearInfoMessage: (expectedInfoMessageId?: InfoMessageId): void => {
      set((state) => {
        if (!state.infoMessage) {
          return;
        }
        if (
          expectedInfoMessageId &&
          state.infoMessage.id !== expectedInfoMessageId
        ) {
          return;
        }
        state.infoMessage = null;
      });
    },
  })),
);
