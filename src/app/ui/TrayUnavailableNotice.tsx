import { useTrayStore } from "@/backend/trayStore";
import { exitApplication } from "@/backend/exitApplication";
import { Button } from "@/components/shared/Button";
import { StatusNotice } from "@/components/shared/StatusNotice";

export function TrayUnavailableNotice(): React.ReactElement | null {
  const status = useTrayStore((state) => state.status);
  const exitStatus = useTrayStore((state) => state.exitStatus);
  const retry = useTrayStore((state) => state.retry);

  if (status === "checking" || status === "available") {
    return null;
  }

  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {status === "unknown" ? (
          <>
            <p>トレイの状態を確認できませんでした。</p>
            <Button size="sm" className="mt-2" onClick={retry}>
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
