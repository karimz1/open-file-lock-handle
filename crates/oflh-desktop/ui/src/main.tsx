import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { applyDocumentLang } from "./i18n";
import "./style.css";
import { trackInputMode } from "./inputMode";
import { applyStartupWindowState } from "./windowStartup";
const stopTrackingInputMode = trackInputMode();
import.meta.hot?.dispose(stopTrackingInputMode);
applyDocumentLang();
// Read once per launch; toggling the setting applies from the next start.
void applyStartupWindowState(localStorage);
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
