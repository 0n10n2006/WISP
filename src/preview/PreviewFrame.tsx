import { useEffect, useState, type ReactNode } from "react";
import { host } from "../services/host";

/**
 * Browser preview only: simulates the desktop around the island window so the
 * UI can be exercised without Tauri. Sizes match the real window.
 */
export function PreviewFrame({ children }: { children: ReactNode }) {
  const [expanded, setExpanded] = useState(false);
  useEffect(() => host().onIslandLayout((l) => setExpanded(l.expanded)), []);
  const call = (fn: string, arg?: string) =>
    (window as unknown as { wispMock: Record<string, (a?: string) => void> }).wispMock[fn]?.(arg);
  return (
    <div className="preview-desktop">
      <div className="preview-note">
        <strong>Wisp browser preview</strong> — simulated host, no network. Try prompts with{" "}
        <code>/quota</code>, <code>/rate</code>, <code>/offline</code>, <code>/slow</code>,{" "}
        <code>/open</code>, <code>/remind</code>, <code>/remember</code>.
        <div className="preview-actions">
          <button onClick={() => call("focus")}>Simulate show shortcut</button>
          <button onClick={() => call("shortcut", "selectToAsk")}>Select to ask</button>
          <button onClick={() => call("shortcut", "dictation")}>Dictation</button>
          <button onClick={() => call("shortcut", "clipboardAssist")}>Clipboard</button>
          <button onClick={() => call("shortcut", "voice")}>Voice</button>
          <button onClick={() => call("reminder")}>Reminder due</button>
          <a href="?window=settings" target="wisp-settings">
            Open settings
          </a>
        </div>
      </div>
      <div
        className="preview-window"
        data-testid="island-window"
        style={expanded ? { width: 400, height: 560 } : { width: 88, height: 88 }}
      >
        {children}
      </div>
    </div>
  );
}
