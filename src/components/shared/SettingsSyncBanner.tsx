import {
  type SettingsApplicationStatus,
  useSettingsStore,
} from "@/store/settingsStore";
import { TimerStatus, useTimerStore } from "@/store/timerStore";
import { WarningTriangleIcon } from "@/components/icons/AppIcons";
import { Button } from "./Button";
import { StatusNotice } from "./StatusNotice";

interface SettingsSyncBannerPresentation {
  tone: "warning" | "danger";
  role: "status" | "alert";
  message: string;
  canRetry: boolean;
}

function canRetrySettingsSynchronization(
  durationStatus: SettingsApplicationStatus,
  automationStatus: SettingsApplicationStatus,
  isConnected: boolean,
  timerStatus: TimerStatus,
  isSynchronizationInFlight: boolean,
): boolean {
  if (
    !isConnected ||
    isSynchronizationInFlight ||
    timerStatus === TimerStatus.ON_BREAK
  ) {
    return false;
  }
  if (automationStatus !== "confirmed") {
    return true;
  }

  return durationStatus !== "confirmed" && timerStatus === TimerStatus.IDLE;
}

function createSettingsSynchronizationWaitMessage(
  durationStatus: SettingsApplicationStatus,
  automationStatus: SettingsApplicationStatus,
  isConnected: boolean,
  timerStatus: TimerStatus,
  isSynchronizationInFlight: boolean,
): string {
  if (isSynchronizationInFlight) {
    return "現在の同期の完了を待っています。";
  }
  if (!isConnected) {
    return "バックエンドへの再接続後に再試行できます。";
  }
  if (timerStatus === TimerStatus.ON_BREAK) {
    if (automationStatus !== "confirmed" && durationStatus !== "confirmed") {
      return "手動での再試行は休憩終了後にできます。時間設定はタイマーがアイドル状態になるまで同期を待ちます。";
    }
    if (automationStatus !== "confirmed") {
      return "手動での再試行は休憩終了後にできます。";
    }
  }
  if (durationStatus === "confirmed" || timerStatus === TimerStatus.IDLE) {
    return "";
  }

  return "時間設定はタイマーがアイドル状態になるまで同期を待ちます。";
}

function createSettingsSyncBannerPresentation(
  durationStatus: SettingsApplicationStatus,
  automationStatus: SettingsApplicationStatus,
  isConnected: boolean,
  timerStatus: TimerStatus,
  isSynchronizationInFlight: boolean,
): SettingsSyncBannerPresentation | null {
  const isDurationUnresolved = durationStatus !== "confirmed";
  const isAutomationUnresolved = automationStatus !== "confirmed";
  if (!isDurationUnresolved && !isAutomationUnresolved) {
    return null;
  }

  const hasUnconfirmedGroup = [durationStatus, automationStatus].includes(
    "unconfirmed",
  );

  const applicationStatusMessage = hasUnconfirmedGroup
    ? "表示中の設定のバックエンドへの適用を確認できませんでした。"
    : "表示中の設定のバックエンドへの適用はまだ確認されていません。";

  const waitMessage = createSettingsSynchronizationWaitMessage(
    durationStatus,
    automationStatus,
    isConnected,
    timerStatus,
    isSynchronizationInFlight,
  );

  return {
    tone: hasUnconfirmedGroup ? "danger" : "warning",
    role: hasUnconfirmedGroup ? "alert" : "status",
    message: [applicationStatusMessage, waitMessage].filter(Boolean).join(" "),
    canRetry: canRetrySettingsSynchronization(
      durationStatus,
      automationStatus,
      isConnected,
      timerStatus,
      isSynchronizationInFlight,
    ),
  };
}

export function SettingsSyncBanner(): React.ReactElement | null {
  const durationStatus = useSettingsStore(
    (state) => state.durationSettingsApplicationStatus,
  );
  const automationStatus = useSettingsStore(
    (state) => state.automationSettingsApplicationStatus,
  );
  const isConnected = useTimerStore((state) => state.isConnected);
  const timerStatus = useTimerStore((state) => state.status);
  const isSynchronizationInFlight = useTimerStore(
    (state) => state.isSettingsSynchronizationInFlight,
  );
  const syncSettings = useTimerStore((state) => state.syncSettings);

  const presentation = createSettingsSyncBannerPresentation(
    durationStatus,
    automationStatus,
    isConnected,
    timerStatus,
    isSynchronizationInFlight,
  );

  if (presentation === null) {
    return null;
  }

  return (
    <StatusNotice
      tone={presentation.tone}
      role={presentation.role}
      ariaLive={presentation.role === "status" ? "polite" : undefined}
      testId="settings-sync-banner"
      icon={
        <WarningTriangleIcon
          className={
            presentation.tone === "danger"
              ? "h-5 w-5 shrink-0 text-danger-400"
              : "h-5 w-5 shrink-0 text-warning-400"
          }
        />
      }
      actions={
        presentation.canRetry ? (
          <Button
            appearance="secondary"
            size="sm"
            onClick={() => {
              void syncSettings().catch(() => undefined);
            }}
          >
            設定の同期を再試行
          </Button>
        ) : undefined
      }
    >
      {presentation.message}
    </StatusNotice>
  );
}
