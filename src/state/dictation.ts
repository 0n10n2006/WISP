import { create } from "zustand";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { AudioRecorder } from "../services/recorder";
import { useActivity } from "./activity";
import { useSettings } from "./settings";
import { NOTHING_HEARD, recorderOptions } from "./voiceOptions";
import { toast } from "./toast";

export type DictationPhase =
  "idle" | "starting" | "listening" | "transcribing" | "review" | "error";

/** Recordings stop automatically after this long. */
export const MAX_DICTATION_MS = 120_000;

interface DictationState {
  phase: DictationPhase;
  level: number;
  startedAt: number;
  transcript: string;
  /** Whether transcription produced any speech (the text may later be edited to empty). */
  heard: boolean;
  error: WispError | null;
  target: { available: boolean; title: string | null };
  /** Listening stops by itself when the user pauses. */
  handsFree: boolean;
  /** `hold`: the user is holding a key, so they decide when it ends. */
  start(hold?: boolean): Promise<void>;
  /** Stop and transcribe; with `autoInsert` the text goes straight into the app. */
  stop(autoInsert?: boolean): Promise<void>;
  /** Shortcut behaviour: start when idle, stop when listening. */
  toggle(): Promise<void>;
  cancel(): void;
  setTranscript(t: string): void;
  insert(): Promise<void>;
  copy(): Promise<void>;
  reset(): void;
}

let recorder: AudioRecorder | null = null;
let autoStop: ReturnType<typeof setTimeout> | null = null;
/** Incremented on cancel so late transcription results are ignored. */
let generation = 0;

const setListening = (v: boolean) => useActivity.getState().setListening(v);

export const useDictation = create<DictationState>((set, get) => ({
  phase: "idle",
  level: 0,
  startedAt: 0,
  transcript: "",
  heard: false,
  error: null,
  handsFree: false,
  target: { available: false, title: null },

  async start(hold = false) {
    if (get().phase === "starting" || get().phase === "listening") return;
    generation += 1;
    set({ phase: "starting", transcript: "", error: null, level: 0 });
    host()
      .dictationTarget()
      .then((target) => set({ target }))
      .catch(() => set({ target: { available: false, title: null } }));
    recorder = host().createRecorder();
    try {
      const options = recorderOptions(
        !hold,
        () => void get().stop(),
        () => {
          get().cancel();
          set({ phase: "error", error: NOTHING_HEARD });
        },
      );
      set({ handsFree: !!options.vad });
      await recorder.start((level) => set({ level }), options);
    } catch (e) {
      recorder = null;
      set({ phase: "error", error: toWispError(e) });
      return;
    }
    setListening(true);
    set({ phase: "listening", startedAt: Date.now() });
    autoStop = setTimeout(() => {
      toast("Stopped after 2 minutes", "neutral");
      void get().stop();
    }, MAX_DICTATION_MS);
  },

  async stop(autoInsert) {
    if (get().phase !== "listening" || !recorder) return;
    if (autoStop) clearTimeout(autoStop);
    const rec = recorder;
    recorder = null;
    setListening(false);
    const gen = generation;
    set({ phase: "transcribing", level: 0 });
    try {
      const recording = await rec.stop();
      if (recording.durationMs < 400) {
        set({ phase: "review", transcript: "", heard: false });
        return;
      }
      const text = await host().transcribe(recording);
      if (gen !== generation) return;
      set({ phase: "review", transcript: text, heard: !!text.trim() });
      const auto =
        autoInsert ?? useSettings.getState().snapshot?.settings.dictationAutoInsert ?? false;
      if (auto && text.trim() && get().target.available) await get().insert();
    } catch (e) {
      if (gen !== generation) return;
      set({ phase: "error", error: toWispError(e) });
    }
  },

  async toggle() {
    const p = get().phase;
    if (p === "listening") return get().stop();
    if (p === "idle" || p === "review" || p === "error") return get().start();
  },

  cancel() {
    generation += 1;
    if (autoStop) clearTimeout(autoStop);
    recorder?.cancel();
    recorder = null;
    setListening(false);
    set({ phase: "idle", level: 0, transcript: "", error: null });
  },

  setTranscript: (transcript) => set({ transcript }),

  async insert() {
    const text = get().transcript.trim();
    if (!text) return;
    try {
      await host().insertText(text);
      set({ phase: "idle", transcript: "" });
      toast("Inserted", "success");
    } catch (e) {
      // Fall back to the clipboard so the text is never lost.
      const err = toWispError(e);
      try {
        await host().copyText(text);
      } catch {
        /* keep the transcript on screen */
      }
      set({ error: err });
    }
  },

  async copy() {
    const text = get().transcript.trim();
    if (!text) return;
    try {
      await host().copyText(text);
      toast("Copied", "success");
      set({ phase: "idle", transcript: "" });
    } catch (e) {
      set({ error: toWispError(e) });
    }
  },

  reset: () => set({ phase: "idle", transcript: "", error: null, level: 0 }),
}));
