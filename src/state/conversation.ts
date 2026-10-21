import { create } from "zustand";
import { host, type AskHandle } from "../services/host";
import { toWispError, type WispError } from "../services/errors";
import type { Attachment, ChatTurn, ContextKind } from "../services/types";
import { SUCCESS_FLASH_MS } from "./orb";

export type MessageStatus = "streaming" | "done" | "stopped" | "failed";

export interface Message {
  id: string;
  role: "user" | "model";
  text: string;
  status: MessageStatus;
  contextKind: ContextKind;
  /** Data URL thumbnail for screen-region questions. */
  imagePreview?: string;
}

export interface AskOptions {
  contextKind?: ContextKind;
  attachments?: Attachment[];
  imagePreview?: string;
  useMemory?: boolean;
}

interface Pending {
  prompt: string;
  opts: AskOptions;
}

interface ConversationState {
  messages: Message[];
  requestActive: boolean;
  error: WispError | null;
  draft: string;
  successFlash: boolean;
  lastFailed: Pending | null;
  setDraft(d: string): void;
  ask(prompt: string, opts?: AskOptions): Promise<void>;
  cancel(): void;
  retry(): Promise<void>;
  dismissError(): void;
  setError(e: WispError): void;
  clear(): void;
}

let active: AskHandle | null = null;
let flashTimer: ReturnType<typeof setTimeout> | null = null;
let seq = 0;

export function newId(prefix = "m"): string {
  seq += 1;
  const rand =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID().slice(0, 8)
      : Math.random().toString(36).slice(2, 10);
  return `${prefix}-${Date.now().toString(36)}-${seq}-${rand}`;
}

function historyFrom(messages: Message[]): ChatTurn[] {
  return messages
    .filter((m) => (m.status === "done" || m.status === "stopped") && m.text.trim())
    .map((m) => ({ role: m.role, text: m.text }));
}

export const useConversation = create<ConversationState>((set, get) => ({
  messages: [],
  requestActive: false,
  error: null,
  draft: "",
  successFlash: false,
  lastFailed: null,

  setDraft: (draft) => set({ draft }),

  async ask(prompt, opts = {}) {
    const text = prompt.trim();
    if ((!text && !opts.attachments?.length) || get().requestActive) return;
    const contextKind = opts.contextKind ?? "text";
    const history = historyFrom(get().messages);
    const userMsg: Message = {
      id: newId(),
      role: "user",
      text,
      status: "done",
      contextKind,
      imagePreview: opts.imagePreview,
    };
    const modelMsg: Message = {
      id: newId(),
      role: "model",
      text: "",
      status: "streaming",
      contextKind,
    };
    if (flashTimer) clearTimeout(flashTimer);
    set((s) => ({
      messages: [...s.messages, userMsg, modelMsg],
      requestActive: true,
      error: null,
      draft: "",
      successFlash: false,
      lastFailed: null,
    }));

    const patchModel = (fn: (m: Message) => Message) =>
      set((s) => ({ messages: s.messages.map((m) => (m.id === modelMsg.id ? fn(m) : m)) }));

    const handle = host().ask(
      {
        requestId: newId("req"),
        prompt: text,
        history,
        attachments: opts.attachments ?? [],
        contextKind,
        useMemory: opts.useMemory ?? true,
        enableTools: true,
      },
      (delta) => patchModel((m) => ({ ...m, text: m.text + delta })),
    );
    active = handle;
    try {
      const result = await handle.result;
      patchModel((m) => ({ ...m, text: result.text || m.text, status: "done" }));
      set({ requestActive: false, successFlash: true });
      flashTimer = setTimeout(() => set({ successFlash: false }), SUCCESS_FLASH_MS);
    } catch (e) {
      const err = toWispError(e);
      const current = get().messages.find((m) => m.id === modelMsg.id);
      const hasText = !!current?.text.trim();
      if (err.code === "cancelled") {
        set((s) => ({
          requestActive: false,
          messages: hasText
            ? s.messages.map((m) => (m.id === modelMsg.id ? { ...m, status: "stopped" } : m))
            : s.messages.filter((m) => m.id !== modelMsg.id && m.id !== userMsg.id),
          // Preserve what the user typed when nothing came back.
          draft: hasText ? s.draft : text,
        }));
        return;
      }
      set((s) => ({
        requestActive: false,
        error: err,
        lastFailed: { prompt: text, opts },
        messages: hasText
          ? s.messages.map((m) => (m.id === modelMsg.id ? { ...m, status: "failed" } : m))
          : s.messages.filter((m) => m.id !== modelMsg.id && m.id !== userMsg.id),
        draft: hasText || s.draft ? s.draft : text,
      }));
    } finally {
      if (active === handle) active = null;
    }
  },

  cancel() {
    active?.cancel();
  },

  async retry() {
    const failed = get().lastFailed;
    if (!failed) return;
    set({ draft: "" });
    await get().ask(failed.prompt, failed.opts);
  },

  dismissError: () => set({ error: null }),
  setError: (error) => set({ error }),

  clear() {
    active?.cancel();
    set({ messages: [], error: null, lastFailed: null, successFlash: false });
  },
}));
