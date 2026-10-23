import { create } from "zustand";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { AudioRecorder } from "../services/recorder";
import { useActivity } from "./activity";
import { useConversation } from "./conversation";
import { useSettings } from "./settings";
import { NOTHING_HEARD, recorderOptions } from "./voiceOptions";

export type VoicePhase = "idle" | "listening" | "transcribing" | "error";

/** Push-to-talk turns are capped to keep requests small. */
export const MAX_VOICE_MS = 60_000;

interface VoiceState {
  phase: VoicePhase;
  level: number;
  error: WispError | null;
  /** Start listening, or finish the turn if already listening. */
  toggle(): Promise<void>;
  /** Listening stops by itself when the user pauses. */
  handsFree: boolean;
  /** `hold`: the user is holding a key, so they decide when it ends. */
  start(hold?: boolean): Promise<void>;
  finish(): Promise<void>;
  cancel(): void;
}

let recorder: AudioRecorder | null = null;
let limit: ReturnType<typeof setTimeout> | null = null;
let generation = 0;
/** True while the microphone is opening; a finish then waits for it. */
let starting = false;
let finishWhenReady = false;

export const useVoice = create<VoiceState>((set, get) => ({
  phase: "idle",
  level: 0,
  error: null,
  handsFree: false,

  async toggle() {
    if (get().phase === "listening") return get().finish();
    if (get().phase === "idle" || get().phase === "error") return get().start();
  },

  async start(hold = false) {
    const settings = useSettings.getState().snapshot?.settings;
    if (settings && !settings.voiceEnabled) {
      set({
        phase: "error",
        error: {
          code: "permission_denied",
          message: "Voice is turned off. Turn it on in Settings → Voice.",
          retryable: false,
        },
      });
      return;
    }
    if (starting || get().phase === "listening" || get().phase === "transcribing") return;
    generation += 1;
    const gen = generation;
    starting = true;
    finishWhenReady = false;
    set({ phase: "listening", error: null, level: 0 });
    const rec = host().createRecorder();
    recorder = rec;
    try {
      const options = recorderOptions(
        !hold,
        () => void get().finish(),
        () => {
          get().cancel();
          set({ phase: "error", error: NOTHING_HEARD });
        },
      );
      set({ handsFree: !!options.vad });
      await rec.start((level) => set({ level }), options);
    } catch (e) {
      starting = false;
      if (recorder === rec) recorder = null;
      // Cancelled while the microphone was opening: nothing to report.
      if (gen === generation) set({ phase: "error", error: toWispError(e) });
      return;
    }
    starting = false;
    if (gen !== generation || recorder !== rec) {
      rec.cancel();
      return;
    }
    useActivity.getState().setListening(true);
    limit = setTimeout(() => void get().finish(), MAX_VOICE_MS);
    if (finishWhenReady) {
      finishWhenReady = false;
      await get().finish();
    }
  },

  async finish() {
    if (starting) {
      // Released before the microphone was ready: finish as soon as it is.
      finishWhenReady = true;
      return;
    }
    if (get().phase !== "listening" || !recorder) return;
    if (limit) clearTimeout(limit);
    const rec = recorder;
    recorder = null;
    useActivity.getState().setListening(false);
    const gen = generation;
    set({ phase: "transcribing", level: 0 });
    try {
      const recording = await rec.stop();
      const text = recording.durationMs < 400 ? "" : await host().transcribe(recording);
      if (gen !== generation) return;
      if (!text.trim()) {
        set({
          phase: "error",
          error: {
            code: "invalid",
            message: "Wisp didn't catch that. Try again and speak a little closer.",
            retryable: true,
          },
        });
        return;
      }
      set({ phase: "idle" });
      // Answers to voice questions are shown as text; Wisp never talks back.
      await useConversation.getState().ask(text, { contextKind: "voice" });
    } catch (e) {
      if (gen !== generation) return;
      set({ phase: "error", error: toWispError(e) });
    }
  },

  cancel() {
    generation += 1;
    starting = false;
    finishWhenReady = false;
    if (limit) clearTimeout(limit);
    recorder?.cancel();
    recorder = null;
    useActivity.getState().setListening(false);
    set({ phase: "idle", level: 0, error: null });
  },
}));
