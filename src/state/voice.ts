// Voice uses Windows' own voice typing (Win+H): it types what you say, live
// and punctuated, into the focused text box. Dictating starts it in the app
// you were using; asking by voice starts it in Wisp's bar, which asks once
// you stop talking. Wisp never records audio itself.

import { create } from "zustand";
import { toWispError } from "../services/errors";
import { host } from "../services/host";
import { useActivity } from "./activity";
import { useNotch } from "./notch";
import { useSettings } from "./settings";
import { toast } from "./toast";

/** Quiet time after the last words before a spoken question is asked. */
export const VOICE_SEND_MS = 1500;

interface VoiceState {
  /** The bar is waiting for a spoken question. */
  listening: boolean;
  /** Voice typing into the app you were using. */
  dictate(): Promise<void>;
  /** Open the bar and voice-type a question into it. */
  ask(): Promise<void>;
  /** Leave listening mode (the question was sent, or the user is editing). */
  stop(): void;
}

function allowed(): boolean {
  if (useSettings.getState().snapshot?.settings.voiceEnabled === false) {
    toast("Voice is turned off. Turn it on in Settings → Voice.", "warning");
    return false;
  }
  return true;
}

const listen = (on: boolean) => useActivity.getState().setListening(on);

export const useVoice = create<VoiceState>((set) => ({
  listening: false,

  async dictate() {
    if (!allowed()) return;
    set({ listening: false });
    listen(false);
    try {
      await host().voiceTyping("app");
    } catch (e) {
      toast(toWispError(e).message, "warning", 5000);
    }
  },

  async ask() {
    if (!allowed()) return;
    useNotch.getState().setQuery("");
    if (useNotch.getState().mode !== "open") {
      useNotch.getState().setMode("open");
      void host()
        .setIslandMode("open")
        .catch(() => {});
    }
    set({ listening: true });
    listen(true);
    try {
      await host().voiceTyping("bar");
    } catch (e) {
      set({ listening: false });
      listen(false);
      toast(toWispError(e).message, "warning", 5000);
    }
  },

  stop() {
    set({ listening: false });
    listen(false);
  },
}));

/** A spoken question as typed by Windows, ready to run: no trailing full stop. */
export function spokenQuery(text: string): string {
  return text
    .trim()
    .replace(/[.。]+$/u, "")
    .trim();
}
