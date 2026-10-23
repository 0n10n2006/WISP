import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initHost } from "./services/host";
import { PreviewFrame } from "./preview/PreviewFrame";
import "./theme/tokens.css";
import "./theme/app.css";
import "./theme/notch.css";

async function main() {
  const h = await initHost();
  // Nothing fails silently: script errors land in the local error log.
  window.addEventListener("error", (e) => h.reportError(`script error: ${e.message}`));
  window.addEventListener("unhandledrejection", (e) => {
    const r: unknown = e.reason;
    h.reportError(
      `unhandled: ${r instanceof Error ? r.message : typeof r === "object" ? JSON.stringify(r) : String(r)}`,
    );
  });
  const root = createRoot(document.getElementById("root")!);
  root.render(
    <StrictMode>
      {h.kind === "mock" && h.windowKind() === "island" ? (
        <PreviewFrame>
          <App />
        </PreviewFrame>
      ) : (
        <App />
      )}
    </StrictMode>,
  );
  requestAnimationFrame(() => h.uiReady());
}

void main();
