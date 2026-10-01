import type { FallbackProps } from "react-error-boundary";
import { Button } from "@/components/shared/Button";

interface SettingsErrorFallbackProps extends FallbackProps {
  onBackToHome: () => void;
}

export function SettingsErrorFallback({
  onBackToHome,
  resetErrorBoundary,
}: SettingsErrorFallbackProps): React.ReactElement {
  const handleBackToHome = (): void => {
    resetErrorBoundary();
    onBackToHome();
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6 text-on-background">
      <section
        role="alert"
        className="flex max-w-xl flex-col items-center gap-4 text-center"
      >
        <h1 className="text-3xl font-semibold text-text">
          設定画面の表示中にエラーが発生しました
        </h1>
        <p className="text-base leading-7 text-text-muted">
          設定画面を再表示するか、ホーム画面に戻って操作を続けてください。
        </p>
        <div className="flex flex-wrap items-center justify-center gap-3">
          <Button appearance="primary" onClick={handleBackToHome}>
            ホームへ戻る
          </Button>
          <Button appearance="secondary" onClick={resetErrorBoundary}>
            再表示を試みる
          </Button>
        </div>
      </section>
    </div>
  );
}
