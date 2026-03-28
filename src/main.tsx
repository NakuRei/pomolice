import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initializeFrontendLogging } from "./logging/logger";
import "./index.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error('Root element with id "root" was not found');
}

const appRootElement = rootElement;

async function bootstrap(): Promise<void> {
  await initializeFrontendLogging();

  ReactDOM.createRoot(appRootElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap().catch((error: unknown) => {
  queueMicrotask(() => {
    throw error;
  });
});
