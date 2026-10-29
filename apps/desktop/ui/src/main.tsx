import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { QuotaPrefsProvider } from "./v2/quota";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QuotaPrefsProvider>
      <App />
    </QuotaPrefsProvider>
  </StrictMode>,
);
