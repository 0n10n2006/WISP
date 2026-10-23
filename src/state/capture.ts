import { create } from "zustand";
import { toWispError } from "../services/errors";
import { host } from "../services/host";
import type { CaptureReady } from "../services/types";
import { useConversation } from "./conversation";
import { useNotch } from "./notch";
import { toast } from "./toast";

/** A screen-region capture waiting for the user to confirm sending it. */
interface CaptureState {
  pending: CaptureReady | null;
  setPending(c: CaptureReady | null): void;
}

export const useCapture = create<CaptureState>((set) => ({
  pending: null,
  setPending: (pending) => set({ pending }),
}));

export const DEFAULT_SCREEN_PROMPT =
  "Describe what's in this part of my screen and explain anything notable.";

/** One-click things to do with a circled area. */
export type CaptureAction = "answer" | "explain" | "search" | "text" | "lens";

const PROMPTS: Record<Exclude<CaptureAction, "lens">, { prompt: string; label: string }> = {
  answer: {
    prompt:
      "Answer the question or solve the problem shown in this part of my screen. Give the answer first, then a brief explanation.",
    label: "Answer this",
  },
  explain: {
    prompt: "Explain what's in this part of my screen in plain words.",
    label: "Explain this",
  },
  search: {
    prompt:
      "Identify what's in this image and search the web for it: what it is, where it comes from, and where to find more (or buy it, if it's a product).",
    label: "Search the web for this",
  },
  text: {
    prompt:
      "Transcribe all the text in this image exactly as written, keeping line breaks. Reply with only the text.",
    label: "Copy the text",
  },
};

/** Run a quick action on the pending capture (and clear it). */
export async function runCaptureAction(action: CaptureAction, web: boolean, think: boolean) {
  const pending = useCapture.getState().pending;
  if (!pending) return;
  if (action === "lens") {
    try {
      const r = await host().reverseImageSearch(pending.dataBase64);
      useCapture.getState().setPending(null);
      toast(
        r.pasted
          ? "Google Lens is open: paste the image with Ctrl+V"
          : "Opened Google Lens results in your browser",
        "success",
        r.pasted ? 5000 : 2500,
      );
    } catch (e) {
      toast(toWispError(e).message, "warning", 4000);
    }
    return;
  }
  const { prompt: base, label } = PROMPTS[action];
  // Anything typed before picking the action is part of the question.
  const typed = useNotch.getState().query.trim();
  const prompt = typed ? `${base}\n\nMy question: ${typed}` : base;
  useCapture.getState().setPending(null);
  useNotch.getState().setQuery("");
  const conversation = useConversation.getState();
  await conversation.ask(prompt, {
    contextKind: "screen",
    attachments: [{ mimeType: pending.mimeType, dataBase64: pending.dataBase64 }],
    imagePreview: pending.dataUrl,
    displayText: typed ? `${label}: ${typed}` : label,
    // Web search is the point of "search"; the others follow the Web toggle.
    web: action === "search" || web,
    think,
  });
  if (action === "text") {
    const last = useConversation.getState().messages.at(-1);
    if (last?.role === "model" && last.status === "done" && last.text.trim()) {
      await host()
        .copyText(last.text.trim())
        .then(() => toast("Text copied", "success"))
        .catch(() => {});
    }
  }
}
