import type { FallbackProps } from "react-error-boundary";
import { Button } from "../shared/Button";

export function FullscreenBreakErrorFallback({
  resetErrorBoundary,
}: FallbackProps): React.ReactElement {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background p-6 text-on-background">
      <section
        role="alert"
        className="flex max-w-xl flex-col items-center gap-4 text-center"
      >
        <h1 className="text-3xl font-semibold text-text">
          表示エラーが発生しました
        </h1>
        <p className="text-base leading-7 text-text-muted">
          休憩タイマーはバックグラウンドで動作しています。休憩が終わると自動的に復帰します。
        </p>
        <Button appearance="secondary" onClick={resetErrorBoundary}>
          再表示を試みる
        </Button>
      </section>
    </div>
  );
}
