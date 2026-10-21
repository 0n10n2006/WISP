import type { RecorderOptions } from "../services/recorder";
import type { VadEvent } from "../lib/vad";
import { useSettings } from "./settings";

/**
 * Microphone options from the user's settings. With `autoStop`, `onDone`
 * runs when they finish talking and `onNothing` when nothing was said.
 */
export function recorderOptions(
  autoStop: boolean,
  onDone: () => void,
  onNothing: () => void,
): RecorderOptions {
  const s = useSettings.getState().snapshot?.settings;
  const opts: RecorderOptions = { deviceId: s?.micDevice || undefined };
  if (autoStop && (s?.autoStopVoice ?? true)) {
    opts.vad = { silenceMs: s?.silenceMs ?? 1200, noSpeechMs: 8000 };
    opts.onVad = (e: VadEvent) => {
      if (e === "end") onDone();
      if (e === "no-speech") onNothing();
    };
  }
  return opts;
}

export const NOTHING_HEARD = {
  code: "invalid" as const,
  message: "Wisp didn't hear anything. Check your microphone and try again.",
  retryable: true,
};
