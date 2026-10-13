import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "./styles.css";
import { Alarm } from "./Alarm";
import { Nudge } from "./Nudge";

// Each native window loads this page with a different hash.
const view = location.hash.replace(/^#/, "") || "alarm";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {view === "nudge" ? <Nudge /> : <Alarm primary={view !== "alarm-secondary"} />}
  </StrictMode>,
);
