// Voice: Wisp listens with Windows' speech recognizer, with no window of its
// own. Dictating types each phrase into the app you're in as it's
// recognised; asking by voice fills Wisp's bar and asks once you pause.
// Wisp never records audio itself.

import { create } from "zustand";
import { playCue } from "../lib/sounds";
import { toWispError } from "../services/errors";
import { host } from "../services/host";
import type { Heard, VoiceTarget } from "../services/types";
import { useActivity } from "./activity";
import { useNotch } from "./notch";
import { useSettings } from "./settings";
import { toast } from "./toast";

/** Words heard this recently mean "you're speaking". */
export const SPEAKING_MS = 700;
/** How long "Got it" / "Didn't catch that" stays in the notch. */
export const OUTCOME_MS = 1400;
/** If Windows doesn't confirm a stop by then, Wisp stops anyway. */
export const STOP_GRACE_MS = 2000;

export type VoiceOutcome = { kind: "done" | "nothing" | "error"; text: string; at: number };

interface VoiceState {
  /** Windows is listening. */
  listening: boolean;
  /** Where the words are going. */
  target: VoiceTarget | null;
  /** The words heard so far. */
  live: string;
  /** When words last arrived (ms), for "hearing you". */
  heardAt: number;
  /** How the last session ended, shown briefly. */
  outcome: VoiceOutcome | null;
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

/** What the user should see: waiting for words, hearing them, or paused. */
export function voicePhase(
  v: Pick<VoiceState, "listening" | "live" | "heardAt">,
  now: number,
): "off" | "waiting" | "hearing" | "paused" {
  if (!v.listening) return "off";
  if (!v.live.trim()) return "waiting";
  return now - v.heardAt < SPEAKING_MS ? "hearing" : "paused";
}

function allowed(): boolean {
  if (useSettings.getState().snapshot?.settings.voiceEnabled === false) {
    toast("Voice is turned off. Turn it on in Settings → Voice.", "warning");
    return false;
  }
  return true;
}

const sounds = () => useSettings.getState().snapshot?.settings.voiceSounds !== false;
const listen = (on: boolean) => useActivity.getState().setListening(on);

function setMode(mode: "sliver" | "peek" | "open") {
  useNotch.getState().setMode(mode);
  void host()
    .setIslandMode(mode)
    .catch(() => {});
}

let stopGuard: ReturnType<typeof setTimeout> | null = null;
let foldTimer: ReturnType<typeof setTimeout> | null = null;
/** Each session's number, so a late event from an old one is ignored. */
let session = 0;

export const useVoice = create<VoiceState>((set, get) => {
  const begin = async (target: VoiceTarget) => {
    const mine = ++session;
    if (foldTimer) clearTimeout(foldTimer);
    set({ listening: true, target, live: "", heardAt: 0, outcome: null, finished: null });
    listen(true);
    if (sounds()) playCue("start");
    try {
      await host().voiceStart(target);
    } catch (e) {
      if (mine !== session) return;
      const message = toWispError(e).message;
      set({
        listening: false,
        target: null,
        outcome: { kind: "error", text: message, at: Date.now() },
      });
      listen(false);
      if (target === "app" && useNotch.getState().mode === "peek") setMode("sliver");
      toast(message, "warning", 6000);
    }
  };
  return {
    listening: false,
    target: null,
    live: "",
    heardAt: 0,
    outcome: null,
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
      const mine = session;
      void host()
        .voiceStop()
        .catch(() => {});
      // Never stay "listening" because Windows didn't answer.
      if (stopGuard) clearTimeout(stopGuard);
      stopGuard = setTimeout(() => {
        const v = get();
        if (mine !== session || !v.listening || !v.target) return;
        v.done({ target: v.target, text: v.live });
        void host()
          .voiceCancel()
          .catch(() => {});
      }, STOP_GRACE_MS);
    },

    cancel() {
      if (!get().listening) return;
      session++;
      set({ listening: false, target: null, live: "" });
      listen(false);
      void host()
        .voiceCancel()
        .catch(() => {});
    },

    heard(h) {
      if (!get().listening) return;
      set({ live: h.text, heardAt: Date.now() });
      if (h.target === "bar") useNotch.getState().setQuery(h.text);
    },

    done(h) {
      if (!get().listening) return;
      if (stopGuard) clearTimeout(stopGuard);
      const text = h.text.trim();
      const kind = h.error && !text ? "error" : text ? "done" : "nothing";
      set({
        listening: false,
        target: null,
        live: "",
        outcome: { kind, text: kind === "error" ? h.error! : text, at: Date.now() },
      });
      listen(false);
      if (sounds()) playCue(kind === "done" ? "done" : "nothing");
      if (h.error) toast(h.error, "warning", 6000);
      if (h.target === "bar") {
        if (text) {
          useNotch.getState().setQuery(h.text);
          set({ finished: h.text });
        }
      } else if (useNotch.getState().mode === "peek") {
        // Show "Got it" (or "Didn't catch that") for a moment, then tuck away.
        if (foldTimer) clearTimeout(foldTimer);
        foldTimer = setTimeout(() => {
          const hovered =
            typeof document !== "undefined" && document.querySelector(".notch-idle:hover") !== null;
          if (!get().listening && !hovered && useNotch.getState().mode === "peek")
            setMode("sliver");
        }, OUTCOME_MS);
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
