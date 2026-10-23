// Instant voice: Windows' speech recognizer turns speech into text while the
// user talks, so words appear live and nothing has to be uploaded and
// transcribed after they stop. The recorder keeps running alongside it; if
// Windows isn't available or isn't sure what it heard, that recording goes
// to Gemini instead (the backup).

import { host } from "../services/host";
import type { SpeechStatus } from "../services/types";
import { useSettings } from "./settings";

export interface LiveSpeech {
  /** The text when Windows is confident, else null (use the recording). */
  stop(): Promise<string | null>;
  cancel(): void;
}

/** How long a "not available" answer is trusted before asking again. */
const STATUS_TTL_MS = 60_000;
let status: { at: number; lang: string; value: SpeechStatus } | null = null;

/** Whether the Windows recognizer can be used (cached briefly). */
export async function speechStatus(lang: string): Promise<SpeechStatus> {
  if (
    status &&
    status.lang === lang &&
    (status.value.available || Date.now() - status.at < STATUS_TTL_MS)
  )
    return status.value;
  const value = await host()
    .speechStatus(lang)
    .catch(() => ({
      available: false,
      reason: "Speech recognition isn't available.",
      fixUri: null,
    }));
  status = { at: Date.now(), lang, value };
  return value;
}

/** Forget the cached status (after the user changes a Windows setting). */
export function resetSpeechStatus(): void {
  status = null;
}

/**
 * Start Windows recognition if the user chose it and it's available;
 * `onWords` receives the words so far. Returns null to use Gemini only.
 */
export async function startLiveSpeech(onWords: (text: string) => void): Promise<LiveSpeech | null> {
  const s = useSettings.getState().snapshot?.settings;
  if ((s?.speechEngine ?? "windows") !== "windows") return null;
  const lang = s?.voiceLanguage ?? "";
  if (!(await speechStatus(lang)).available) return null;
  const off = host().onSpeech(onWords);
  try {
    await host().speechStart(lang);
  } catch {
    off();
    return null;
  }
  let done = false;
  return {
    async stop() {
      if (done) return null;
      done = true;
      try {
        const heard = await host().speechStop();
        const text = heard.text.trim();
        return text && (heard.confidence === "high" || heard.confidence === "medium") ? text : null;
      } catch {
        return null;
      } finally {
        off();
      }
    },
    cancel() {
      if (done) return;
      done = true;
      off();
      void host()
        .speechCancel()
        .catch(() => {});
    },
  };
}
