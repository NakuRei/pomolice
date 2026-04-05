import type { CSSProperties } from "react";
import type { FallbackProps } from "react-error-boundary";

const containerStyle: CSSProperties = {
  minHeight: "100vh",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  padding: "24px",
  backgroundColor: "#111827",
  color: "#f9fafb",
};

const contentStyle: CSSProperties = {
  width: "100%",
  maxWidth: "480px",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: "16px",
  textAlign: "center",
};

const titleStyle: CSSProperties = {
  margin: 0,
  fontSize: "32px",
  fontWeight: 700,
  lineHeight: 1.2,
};

const descriptionStyle: CSSProperties = {
  margin: 0,
  color: "#d1d5db",
  fontSize: "16px",
  lineHeight: 1.6,
};

const buttonStyle: CSSProperties = {
  border: 0,
  borderRadius: "12px",
  padding: "12px 20px",
  backgroundColor: "#2563eb",
  color: "#f9fafb",
  fontSize: "16px",
  fontWeight: 600,
  cursor: "pointer",
};

export function AppErrorFallback({
  resetErrorBoundary,
}: FallbackProps): React.ReactElement {
  return (
    <div style={containerStyle}>
      <section role="alert" style={contentStyle}>
        <h1 style={titleStyle}>アプリケーションエラーが発生しました</h1>
        <p style={descriptionStyle}>
          画面の再表示を試みてください。復帰しない場合はアプリを再起動してください。
        </p>
        <button type="button" style={buttonStyle} onClick={resetErrorBoundary}>
          再表示を試みる
        </button>
      </section>
    </div>
  );
}
