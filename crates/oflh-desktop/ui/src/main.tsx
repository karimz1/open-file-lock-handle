import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { locale } from "./i18n";
import "./style.css";
import { trackInputMode } from "./inputMode";
import { applyStartupWindowState } from "./windowStartup";
const stopTrackingInputMode = trackInputMode();
import.meta.hot?.dispose(stopTrackingInputMode);
document.documentElement.lang = locale;
// Read once per launch; toggling the setting applies from the next start.
void applyStartupWindowState(localStorage);
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
