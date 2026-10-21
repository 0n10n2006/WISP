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
