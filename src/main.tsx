import ReactDOM from "react-dom/client";
import { AppRoot } from "./AppRoot";
import { initializeFrontendLogDevtools } from "./logging/devtools";
import { resolveCreateRootOptions } from "./logging/createRootErrorCallbacks";
import "./index.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error('Root element with id "root" was not found');
}

void initializeFrontendLogDevtools();

const rootOptions = resolveCreateRootOptions(import.meta.env.PROD);

ReactDOM.createRoot(rootElement, rootOptions).render(<AppRoot />);
