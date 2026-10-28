// Voice: Wisp listens with Windows' speech recognizer, with no window of its
// own. Dictating types each phrase into the app you're in as it's
// recognised; asking by voice fills Wisp's bar and asks once you pause.
// Wisp never records audio itself.

import { create } from "zustand";
import { toWispError } from "../services/errors";
import { host } from "../services/host";
import type { Heard, VoiceTarget } from "../services/types";
import { useActivity } from "./activity";
import { useNotch } from "./notch";
import { useSettings } from "./settings";
import { toast } from "./toast";

interface VoiceState {
  /** Windows is listening. */
  listening: boolean;
  /** Where the words are going. */
  target: VoiceTarget | null;
  /** The words heard so far. */
  live: string;
  /** A spoken question that just finished, for the bar to ask (then cleared). */
  finished: string | null;
  /** Dictate into the app you're in (again: stop). */
  dictate(): Promise<void>;
  /** Ask by voice in Wisp's bar (again: stop). */
  ask(): Promise<void>;
  /** Stop listening now, keeping what was heard. */
  stop(): void;
  /** Stop listening and drop it (e.g. the user started typing). */
  cancel(): void;
  heard(h: Heard): void;
  done(h: Heard): void;
  takeFinished(): string | null;
}

function allowed(): boolean {
  if (useSettings.getState().snapshot?.settings.voiceEnabled === false) {
    toast("Voice is turned off. Turn it on in Settings → Voice.", "warning");
    return false;
  }
  return true;
}

const listen = (on: boolean) => useActivity.getState().setListening(on);

function setMode(mode: "sliver" | "peek" | "open") {
  useNotch.getState().setMode(mode);
  void host()
    .setIslandMode(mode)
    .catch(() => {});
}

export const useVoice = create<VoiceState>((set, get) => {
  const begin = async (target: VoiceTarget) => {
    set({ listening: true, target, live: "", finished: null });
    listen(true);
    try {
      await host().voiceStart(target);
    } catch (e) {
      set({ listening: false, target: null });
      listen(false);
      if (target === "app" && useNotch.getState().mode === "peek") setMode("sliver");
      toast(toWispError(e).message, "warning", 6000);
    }
  };
  return {
    listening: false,
    target: null,
    live: "",
    finished: null,

    async dictate() {
      if (get().listening) return get().stop();
      if (!allowed()) return;
      // Show what's heard in the notch without taking focus from the app.
      if (useNotch.getState().mode === "sliver") setMode("peek");
      await begin("app");
    },

    async ask() {
      if (get().listening) return get().stop();
      if (!allowed()) return;
      useNotch.getState().setQuery("");
      if (useNotch.getState().mode !== "open") setMode("open");
      await begin("bar");
    },

    stop() {
      if (!get().listening) return;
      void host()
        .voiceStop()
        .catch(() => {});
    },

    cancel() {
      if (!get().listening) return;
      set({ listening: false, target: null, live: "" });
      listen(false);
      void host()
        .voiceCancel()
        .catch(() => {});
    },

    heard(h) {
      if (!get().listening) return;
      set({ live: h.text });
      if (h.target === "bar") useNotch.getState().setQuery(h.text);
    },

    done(h) {
      if (!get().listening) return;
      set({ listening: false, target: null, live: "" });
      listen(false);
      if (h.target === "bar") {
        if (h.text.trim()) {
          useNotch.getState().setQuery(h.text);
          set({ finished: h.text });
        }
      } else if (useNotch.getState().mode === "peek") {
        const hovered =
          typeof document !== "undefined" && document.querySelector(".notch-idle:hover") !== null;
        if (!hovered) setMode("sliver");
      }
    },

    takeFinished() {
      const f = get().finished;
      if (f !== null) set({ finished: null });
      return f;
    },
  };
});

/** A spoken question as recognised, ready to run: no trailing full stop. */
export function spokenQuery(text: string): string {
  return text
    .trim()
    .replace(/[.。]+$/u, "")
    .trim();
}
