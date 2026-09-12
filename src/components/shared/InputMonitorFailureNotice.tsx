import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Button } from "./Button";
import { StatusNotice } from "./StatusNotice";

export function InputMonitorFailureNotice(): React.ReactElement | null {
  const [failureObserved, setFailureObserved] = useState(false);
  const [connectionFailed, setConnectionFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined = undefined;

    function acceptSnapshot(
      observed: boolean,
      notificationsAvailable: boolean,
    ): void {
      if (active) {
        setFailureObserved((previous) => previous || observed);
        setConnectionFailed(!notificationsAvailable);
      }
    }

    async function connect(): Promise<void> {
      let notificationsAvailable = false;
      try {
        const unsubscribe = await listen("input_monitor_failed", () => {
          if (active) setFailureObserved(true);
        });
        if (!active) {
          unsubscribe();
          return;
        }
        unlisten = unsubscribe;
        notificationsAvailable = true;
      } catch {
        if (active) setConnectionFailed(true);
      }

      try {
        const observed = await invoke<boolean>("has_input_monitor_failed");
        acceptSnapshot(observed, notificationsAvailable);
      } catch {
        if (active) setConnectionFailed(true);
      }
    }

    void connect();
    return (): void => {
      active = false;
      unlisten?.();
    };
  }, [attempt]);

  if (!failureObserved && !connectionFailed) return null;

  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {failureObserved ? (
          <>
            <p>入力監視でエラーが発生しました。</p>
            <p>
              休憩中の操作へのペナルティと緊急解除キー（Ctrl+Alt+Shift+E）が働かない可能性があります。
            </p>
          </>
        ) : (
          <>
            <p>入力監視のエラー情報を取得できませんでした。</p>
            <Button
              size="sm"
              className="mt-2"
              onClick={() => {
                setAttempt((previous) => previous + 1);
              }}
            >
              再確認
            </Button>
          </>
        )}
      </StatusNotice>
    </div>
  );
}
