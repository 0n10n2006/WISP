import { create } from "zustand";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { Task } from "../services/types";

export const MAX_CLIPBOARD_CHARS = 20_000;

interface ClipboardState {
  open: boolean;
  loading: boolean;
  text: string | null;
  error: WispError | null;
  /** Read the clipboard once, on an explicit user action. */
  openAssist(): Promise<void>;
  close(): void;
}

export const useClipboardAssist = create<ClipboardState>((set) => ({
  open: false,
  loading: false,
  text: null,
  error: null,
  async openAssist() {
    set({ open: true, loading: true, text: null, error: null });
    try {
      set({ text: await host().clipboardRead(), loading: false });
    } catch (e) {
      set({ error: toWispError(e), loading: false });
    }
  },
  close: () => set({ open: false, text: null, error: null, loading: false }),
}));

/**
 * One-tap help with the clipboard (from an app-aware chip). The clipboard is
 * read only now, on this click, and the request shows a short label.
 */
export async function askAboutClipboard(prompt: string, label: string): Promise<void> {
  const { useConversation } = await import("./conversation");
  let text: string | null;
  try {
    text = await host().clipboardRead();
  } catch (e) {
    useConversation.getState().setError(toWispError(e));
    return;
  }
  if (!text?.trim()) {
    const { toast } = await import("./toast");
    toast("Copy some text first, then tap the chip again", "warning", 3000);
    return;
  }
  const clipped = text.slice(0, MAX_CLIPBOARD_CHARS);
  await useConversation.getState().ask(`${prompt}\n\n---\n${clipped}`, {
    contextKind: "clipboard",
    displayText: `${label} — clipboard text (${clipped.length.toLocaleString()} characters)`,
  });
}

interface RemindersState {
  due: Task[];
  push(t: Task): void;
  dismiss(id: string): void;
}

export const useDueReminders = create<RemindersState>((set) => ({
  due: [],
  push: (t) => set((s) => ({ due: [...s.due.filter((x) => x.id !== t.id), t] })),
  dismiss: (id) => set((s) => ({ due: s.due.filter((x) => x.id !== id) })),
}));
