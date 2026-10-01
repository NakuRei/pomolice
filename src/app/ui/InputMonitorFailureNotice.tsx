import { useInputMonitorStore } from "@/backend/inputMonitorStore";
import { Button } from "@/components/shared/Button";
import { StatusNotice } from "@/components/shared/StatusNotice";
import { useEmergencyShortcutStore } from "@/backend/emergencyShortcutStore";

export function InputMonitorFailureNotice(): React.ReactElement | null {
  const usesInputMonitorShortcut = useEmergencyShortcutStore(
    (state) => state.status?.kind === "input_monitor",
  );
  const failureObserved = useInputMonitorStore(
    (state) => state.failureObserved,
  );
  const connectionFailed = useInputMonitorStore(
    (state) => state.connectionFailed,
  );
  const retry = useInputMonitorStore((state) => state.retry);

  if (!failureObserved && !connectionFailed) return null;

  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {failureObserved ? (
          <>
            <p>入力監視でエラーが発生しました。</p>
            <p>
              {usesInputMonitorShortcut
                ? "休憩中の操作へのペナルティと緊急解除キー（Ctrl+Alt+Shift+E）が働かない可能性があります。"
                : "休憩中の操作へのペナルティが働かない可能性があります。"}
            </p>
          </>
        ) : (
          <>
            <p>入力監視のエラー情報を取得できませんでした。</p>
            <Button size="sm" className="mt-2" onClick={retry}>
              再確認
            </Button>
          </>
        )}
      </StatusNotice>
    </div>
  );
}
