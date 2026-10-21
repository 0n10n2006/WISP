import { useEffect, useState, type ReactNode } from "react";
import { host } from "../services/host";
import type { IslandLayout } from "../services/types";

/** Logical window sizes, mirroring `src-tauri/src/windows.rs`. */
const SIZES = { sliver: [180, 10], peek: [380, 46], open: [760, 0] } as const;

/**
 * Browser preview only: simulates the desktop around the notch window so the
 * UI can be exercised without Tauri. Sizes match the real window.
 */
export function PreviewFrame({ children }: { children: ReactNode }) {
  const [layout, setLayout] = useState<IslandLayout>({
    mode: "sliver",
    expanded: false,
    height: 0,
  });
  useEffect(() => host().onIslandLayout(setLayout), []);
  const call = (fn: string, arg?: unknown) =>
    (window as unknown as { wispMock: Record<string, (a?: unknown) => void> }).wispMock[fn]?.(arg);
  const [w, h] = SIZES[layout.mode];
  return (
    <div className="preview-desktop">
      <div
        className="preview-window"
        data-testid="island-window"
        data-mode={layout.mode}
        style={{ width: w, height: layout.mode === "open" ? Math.max(96, layout.height) : h }}
      >
        {children}
      </div>
      <div className="preview-note">
        <strong>Wisp browser preview</strong> — simulated host, no network. Hover the glow at the
        top, or try prompts with <code>/quota</code>, <code>/rate</code>, <code>/offline</code>,{" "}
        <code>/slow</code>, <code>/open</code>, <code>/remind</code>, <code>/remember</code>, and
        commands like <code>timer 25</code>, <code>volume 30</code>, <code>spotify</code>.
        <div className="preview-actions">
          <button onClick={() => call("focus")}>Open command bar</button>
          <button onClick={() => call("shortcut", "selectToAsk")}>Circle to ask</button>
          <button onClick={() => call("hold", { chord: "dictate", phase: "start" })}>
            Hold Ctrl+Win
          </button>
          <button onClick={() => call("hold", { chord: "dictate", phase: "stop" })}>Release</button>
          <button onClick={() => call("shortcut", "clipboardAssist")}>Clipboard</button>
          <button onClick={() => call("reminder")}>Reminder due</button>
          <button onClick={() => call("blur")}>Click elsewhere</button>
          <a href="?window=settings" target="wisp-settings">
            Open settings
          </a>
        </div>
      </div>
    </div>
  );
}
