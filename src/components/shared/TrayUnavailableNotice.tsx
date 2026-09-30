import { commands } from "@/ipc/generated";
import { useEffect, useState } from "react";
import { Button } from "./Button";
import { StatusNotice } from "./StatusNotice";

export function TrayUnavailableNotice(): React.ReactElement | null {
  const [status, setStatus] = useState<
    "checking" | "available" | "unavailable" | "unknown"
  >("checking");
  const [checkAttempt, setCheckAttempt] = useState(0);
  const [exitStatus, setExitStatus] = useState<"idle" | "exiting" | "failed">(
    "idle",
  );

  useEffect(() => {
    let active = true;
    void commands.isTrayAvailable().then(
      (available) => {
        if (active) {
          setStatus(available ? "available" : "unavailable");
        }
      },
      () => {
        if (active) {
          setStatus("unknown");
        }
      },
    );
    return (): void => {
      active = false;
    };
  }, [checkAttempt]);

  const exitApplication = async (): Promise<void> => {
    setExitStatus("exiting");
    try {
      await commands.exitWithoutTray();
    } catch {
      setExitStatus("failed");
    }
  };

  if (status === "checking" || status === "available") {
    return null;
  }

  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {status === "unknown" ? (
          <>
            <p>トレイの状態を確認できませんでした。</p>
            <Button
              size="sm"
              className="mt-2"
              onClick={() => {
                setStatus("checking");
                setCheckAttempt((attempt) => attempt + 1);
              }}
            >
              再確認
            </Button>
          </>
        ) : (
          <>
            <p className="font-semibold">トレイを初期化できませんでした。</p>
            <p>
              トレイから再表示できないため、閉じるボタンを押しても時計は非表示になりません。タイマーは引き続き利用できます。
            </p>
            <Button
              appearance="danger"
              size="sm"
              className="mt-2"
              disabled={exitStatus === "exiting"}
              onClick={() => {
                void exitApplication();
              }}
            >
              {exitStatus === "exiting" ? "終了中…" : "アプリを終了"}
            </Button>
            {exitStatus === "failed" ? (
              <p className="mt-2">
                終了できませんでした。もう一度お試しください。
              </p>
            ) : null}
          </>
        )}
      </StatusNotice>
    </div>
  );
}
