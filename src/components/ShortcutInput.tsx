import { useEffect, useState, type KeyboardEvent } from "react";
import { host } from "../services/host";

const CODE_KEYS: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "ArrowUp",
  ArrowDown: "ArrowDown",
  ArrowLeft: "ArrowLeft",
  ArrowRight: "ArrowRight",
  Period: "Period",
  Comma: "Comma",
  Slash: "Slash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Minus: "Minus",
  Equal: "Equal",
  Backquote: "Backquote",
};

/** Turn a keydown into a Tauri accelerator string, or null for bare modifiers. */
export function acceleratorFromEvent(e: {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}): string | null {
  let key: string | undefined;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) key = e.code;
  else key = CODE_KEYS[e.code];
  if (!key) return null;
  const mods = [
    e.ctrlKey && "Ctrl",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
    e.metaKey && "Super",
  ].filter(Boolean);
  return [...mods, key].join("+");
}

interface Props {
  id: string;
  label: string;
  value: string;
  onChange(v: string): void;
}

export function ShortcutInput({ id, label, value, onChange }: Props) {
  const [recording, setRecording] = useState(false);
  // While recording, Wisp's own shortcuts step aside so the keys are captured.
  useEffect(() => {
    if (!recording) return;
    host().pauseShortcuts(true);
    return () => host().pauseShortcuts(false);
  }, [recording]);

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        setRecording(true);
      }
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      setRecording(false);
      return;
    }
    if ((e.key === "Backspace" || e.key === "Delete") && !e.ctrlKey && !e.altKey && !e.metaKey) {
      onChange("");
      setRecording(false);
      return;
    }
    const accel = acceleratorFromEvent(e.nativeEvent);
    if (accel) {
      onChange(accel);
      setRecording(false);
    }
  };

  return (
    <button
      id={id}
      type="button"
      className={`shortcut-input ${recording ? "recording" : ""}`}
      aria-label={`${label}: ${value || "disabled"}. ${recording ? "Press the new shortcut, Esc to cancel, Backspace to disable." : "Press Enter to change."}`}
      onClick={() => setRecording((r) => !r)}
      onKeyDown={onKeyDown}
      onBlur={() => setRecording(false)}
    >
      {recording ? (
        "Press keys…"
      ) : value ? (
        <kbd>{value}</kbd>
      ) : (
        <span className="muted">Disabled</span>
      )}
    </button>
  );
}
