import { create } from "zustand";
import { host, type AskHandle } from "../services/host";
import { toWispError, type WispError } from "../services/errors";
import type { Attachment, ChatTurn, ContextKind, Source } from "../services/types";
import { SUCCESS_FLASH_MS } from "./orb";
import { useSettings } from "./settings";
import { toast } from "./toast";

export type MessageStatus = "streaming" | "done" | "stopped" | "failed";

export interface ActionNote {
  /** Present while waiting for the user's decision. */
  confirmId?: string;
  kind: string;
  description: string;
  status: "pending" | "done" | "scheduled" | "declined" | "rejected" | "failed";
  message?: string;
}

export interface Message {
  id: string;
  role: "user" | "model";
  text: string;
  status: MessageStatus;
  contextKind: ContextKind;
  /** Data URL thumbnail for screen-region questions. */
  imagePreview?: string;
  /** The full prompt when `text` shows a shorter label (e.g. clipboard requests). */
  sentText?: string;
  /** Desktop actions the model proposed during this answer. */
  actions?: ActionNote[];
  /** Web pages a grounded answer drew on. */
  sources?: Source[];
  /** The question was answered with Google Search. */
  web?: boolean;
  /** Another model than the chosen one answered (its readable name). */
  via?: string;
  /** Something to know about this answer (e.g. answered without web search). */
  note?: string;
}

export interface AskOptions {
  contextKind?: ContextKind;
  attachments?: Attachment[];
  imagePreview?: string;
  useMemory?: boolean;
  /** What to show in the user's bubble when the prompt itself is long. */
  displayText?: string;
  /** Ground the answer with Google Search. */
  web?: boolean;
  /** Let the model think longer. */
  think?: boolean;
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
  /** Per-conversation memory opt-out (FR-09). Resets with a new conversation. */
  memoryForChat: boolean;
  setMemoryForChat(v: boolean): void;
  setDraft(d: string): void;
  ask(prompt: string, opts?: AskOptions): Promise<void>;
  cancel(): void;
  retry(): Promise<void>;
  respondAction(confirmId: string, approved: boolean): void;
  dismissError(): void;
  setError(e: WispError): void;
  clear(): void;
}

let active: AskHandle | null = null;

const scheduleFrame = (fn: () => void): number =>
  typeof requestAnimationFrame === "function"
    ? requestAnimationFrame(fn)
    : (setTimeout(fn, 16) as unknown as number);
const cancelFrame = (id: number | null) => {
  if (id === null) return;
  if (typeof cancelAnimationFrame === "function") cancelAnimationFrame(id);
  else clearTimeout(id);
};
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
    .map((m) => ({ role: m.role, text: m.sentText ?? m.text }));
}

export const useConversation = create<ConversationState>((set, get) => ({
  messages: [],
  requestActive: false,
  error: null,
  draft: "",
  successFlash: false,
  lastFailed: null,
  memoryForChat: true,

  setMemoryForChat: (memoryForChat) => set({ memoryForChat }),
  setDraft: (draft) => set({ draft }),

  async ask(prompt, opts = {}) {
    const text = prompt.trim();
    if ((!text && !opts.attachments?.length) || get().requestActive) return;
    const contextKind = opts.contextKind ?? "text";
    const history = historyFrom(get().messages);
    const userMsg: Message = {
      id: newId(),
      role: "user",
      text: opts.displayText ?? text,
      sentText: opts.displayText ? text : undefined,
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
      web: opts.web,
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

    // Streamed text is applied at most once per frame so long answers stay smooth.
    let buffered = "";
    let frame: number | null = null;
    const flush = () => {
      frame = null;
      if (!buffered) return;
      const chunk = buffered;
      buffered = "";
      patchModel((m) => ({ ...m, text: m.text + chunk }));
    };
    const onDelta = (delta: string) => {
      buffered += delta;
      if (frame === null) frame = scheduleFrame(flush);
    };

    const handle = host().ask(
      {
        requestId: newId("req"),
        prompt: text,
        history,
        attachments: opts.attachments ?? [],
        contextKind,
        useMemory: opts.useMemory ?? get().memoryForChat,
        enableTools: true,
        think: opts.think ?? false,
        web: opts.web ?? false,
        outsideContent: get().messages.some(
          (m) => m.contextKind === "screen" || m.contextKind === "clipboard" || m.web,
        ),
      },
      onDelta,
      (ev) =>
        patchModel((m) => {
          const notes = [...(m.actions ?? [])];
          if (ev.type === "actionProposed") {
            notes.push({
              confirmId: ev.id,
              kind: ev.kind,
              description: ev.description,
              status: "pending",
            });
          } else {
            const i = notes.findIndex((n) => n.kind === ev.kind && n.status === "pending");
            const note: ActionNote = {
              kind: ev.kind,
              description: ev.description,
              status: ev.outcome,
              message: ev.message,
            };
            if (i >= 0) notes[i] = note;
            else notes.push(note);
          }
          return { ...m, actions: notes };
        }),
    );
    active = handle;
    try {
      const result = await handle.result;
      cancelFrame(frame);
      flush();
      const chosen = useSettings.getState().snapshot?.settings.selectedModel;
      const via =
        result.model && chosen && result.model !== chosen
          ? result.modelLabel || result.model
          : undefined;
      patchModel((m) => ({
        ...m,
        text: result.text || m.text,
        status: "done",
        sources: result.sources?.length ? result.sources : undefined,
        via,
        note: result.note || undefined,
      }));
      if (result.fellBack && result.skipped?.length)
        toast(
          `${result.skipped.join(", ")} is used up or busy, so ${result.modelLabel || result.model} answered.`,
          "neutral",
          4500,
        );
      set({ requestActive: false, successFlash: true });
      flashTimer = setTimeout(() => set({ successFlash: false }), SUCCESS_FLASH_MS);
    } catch (e) {
      cancelFrame(frame);
      flush();
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
          draft: hasText || opts.displayText ? s.draft : text,
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
        draft: hasText || s.draft || opts.displayText ? s.draft : text,
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

  respondAction(confirmId, approved) {
    set((s) => ({
      messages: s.messages.map((m) => ({
        ...m,
        actions: m.actions?.map((a) =>
          a.confirmId === confirmId
            ? { ...a, confirmId: undefined, status: approved ? "pending" : "declined" }
            : a,
        ),
      })),
    }));
    void host()
      .respondAction(confirmId, approved)
      .catch(() => {
        // The host no longer knows this request (it expired): say so.
        set((s) => ({
          messages: s.messages.map((m) => ({
            ...m,
            actions: m.actions?.map((a) =>
              a.status === "pending" && !a.confirmId ? { ...a, status: "failed" } : a,
            ),
          })),
        }));
      });
  },

  dismissError: () => set({ error: null }),
  setError: (error) => set({ error }),

  clear() {
    active?.cancel();
    set({ messages: [], error: null, lastFailed: null, successFlash: false, memoryForChat: true });
  },
}));
