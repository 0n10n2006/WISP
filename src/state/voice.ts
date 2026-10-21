import { create } from "zustand";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { AudioRecorder } from "../services/recorder";
import { speak, stopSpeaking } from "../services/speech";
import { useActivity } from "./activity";
import { useConversation } from "./conversation";
import { useSettings } from "./settings";

export type VoicePhase = "idle" | "listening" | "transcribing" | "error";

/** Push-to-talk turns are capped to keep requests small. */
export const MAX_VOICE_MS = 60_000;

interface VoiceState {
  phase: VoicePhase;
  level: number;
  error: WispError | null;
  /** Start listening, or finish the turn if already listening. */
  toggle(): Promise<void>;
  start(): Promise<void>;
  finish(): Promise<void>;
  cancel(): void;
}

let recorder: AudioRecorder | null = null;
let limit: ReturnType<typeof setTimeout> | null = null;
let generation = 0;

/** Read the latest answer aloud with the user's voice settings. */
export async function speakLatestAnswer(): Promise<void> {
  const msgs = useConversation.getState().messages;
  const last = [...msgs].reverse().find((m) => m.role === "model" && m.status === "done");
  if (!last?.text) return;
  const s = useSettings.getState().snapshot?.settings;
  await speak(last.text, { voice: s?.ttsVoice || undefined, rate: s?.ttsRate ?? 1 });
}

export const useVoice = create<VoiceState>((set, get) => ({
  phase: "idle",
  level: 0,
  error: null,

  async toggle() {
    if (get().phase === "listening") return get().finish();
    if (get().phase === "idle" || get().phase === "error") return get().start();
  },

  async start() {
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
    // Pressing talk interrupts any answer being read aloud.
    stopSpeaking();
    generation += 1;
    set({ phase: "listening", error: null, level: 0 });
    recorder = host().createRecorder();
    try {
      await recorder.start((level) => set({ level }));
    } catch (e) {
      recorder = null;
      set({ phase: "error", error: toWispError(e) });
      return;
    }
    useActivity.getState().setListening(true);
    limit = setTimeout(() => void get().finish(), MAX_VOICE_MS);
  },

  async finish() {
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
      await useConversation.getState().ask(text, { contextKind: "voice" });
      if (gen !== generation || useConversation.getState().error) return;
      await speakLatestAnswer();
    } catch (e) {
      if (gen !== generation) return;
      set({ phase: "error", error: toWispError(e) });
    }
  },

  cancel() {
    generation += 1;
    if (limit) clearTimeout(limit);
    recorder?.cancel();
    recorder = null;
    useActivity.getState().setListening(false);
    stopSpeaking();
    set({ phase: "idle", level: 0, error: null });
  },
}));
