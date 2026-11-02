import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { QuotaPrefsProvider } from "./v2/quota";
import { Popover } from "./v2/Popover";
import "./v2/theme.css";
import "./v2/popover.css";
import "./styles.css";

// The menu bar popover (T-0019) loads the same page with "#tray".
const tray = window.location.hash === "#tray";
if (tray) document.documentElement.classList.add("tray");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QuotaPrefsProvider>{tray ? <Popover /> : <App />}</QuotaPrefsProvider>
  </StrictMode>,
);
