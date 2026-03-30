import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initializeFrontendLogDevtools } from "./logging/devtools";
import "./index.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error('Root element with id "root" was not found');
}

void initializeFrontendLogDevtools();

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
