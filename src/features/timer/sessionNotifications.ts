import type { SessionType, SessionCompletedPayload } from "@/ipc/generated";
import { useBackendObservationStore } from "@/backend/observationStore";
import {
  sendSystemNotification,
  type NotificationSoundPreferences,
} from "@/services/notifications";

function readSoundPreferences(): NotificationSoundPreferences | null {
  const { settings } = useBackendObservationStore.getState();
  return settings === null
    ? null
    : {
        muted: settings.notificationSoundMuted,
        volumePercent: settings.notificationSoundVolumePercent,
      };
}

/**
 * Show notification when work session is complete
 */
export async function notifyWorkComplete(
  nextSessionType: SessionType | null,
): Promise<void> {
  const nextSessionSoundType =
    nextSessionType === "long_break" ? "long_break" : "short_break";

  await sendSystemNotification(
    {
      title: "休憩時間です！",
      body: "お疲れ様でした。休憩を取りましょう。",
      soundType: nextSessionSoundType,
    },
    readSoundPreferences,
  );
}

/**
 * Show notification when break session is complete
 */
export async function notifyBreakComplete(): Promise<void> {
  await sendSystemNotification(
    {
      title: "作業に戻りましょう！",
      body: "休憩が終わりました。次のポモドーロを始めましょう。",
      soundType: "work",
    },
    readSoundPreferences,
  );
}

/**
 * Show notification when break session auto-starts the next work session.
 */
export async function notifyBreakAutoStarted(): Promise<void> {
  await sendSystemNotification(
    {
      title: "休憩終了",
      body: "休憩終了。次の作業を自動開始しました。",
      soundType: "work",
    },
    readSoundPreferences,
  );
}

/**
 * Show notification when long break session is complete
 */
export async function notifyLongBreakComplete(): Promise<void> {
  await sendSystemNotification(
    {
      title: "長い休憩完了！",
      body: "サイクルが完了しました。次のサイクルを始めますか？",
      soundType: "work",
    },
    readSoundPreferences,
  );
}

export function notifySessionCompleted(
  payload: SessionCompletedPayload,
): Promise<void> {
  switch (payload.transition_reason) {
    case "work_to_break":
      return notifyWorkComplete(payload.next_session_type);
    case "break_waiting_decision":
      return notifyBreakComplete();
    case "break_auto_started":
      return notifyBreakAutoStarted();
    case "scheduled_stop_executed":
    default:
      return Promise.resolve();
  }
}
