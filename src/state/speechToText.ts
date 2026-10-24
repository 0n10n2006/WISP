// Turning a recording into text. Whisper runs on this PC by default: it's
// accurate, works offline and nothing is uploaded. Until its model is
// downloaded (that starts in the background the first time voice is used),
// or if it fails, the recording goes to Gemini instead.

import { toWispError } from "../services/errors";
import { host } from "../services/host";
import type { Recording } from "../services/recorder";
import type { WhisperModel } from "../services/types";
import { useSettings } from "./settings";
import { toast } from "./toast";

export type Purpose = "dictation" | "voice";

/** Models this session already started downloading (once each). */
const fetching = new Set<WhisperModel>();

function whisperFor(purpose: Purpose): boolean {
  const s = useSettings.getState().snapshot?.settings;
  if ((s?.speechEngine ?? "whisper") !== "whisper") return false;
  // Formal dictation needs rewording, which only Gemini does.
  return !(purpose === "dictation" && s?.dictationStyle === "formal");
}

/** Start the model download in the background, once per session. */
export function fetchModel(model: WhisperModel): void {
  if (fetching.has(model)) return;
  fetching.add(model);
  toast("Downloading the speech model for offline voice…", "neutral");
  host()
    .whisperDownload(model)
    .then(() => toast("Offline voice is ready", "success"))
    .catch((e) => {
      fetching.delete(model);
      const err = toWispError(e);
      if (err.code !== "cancelled") toast(err.message, "warning");
    });
}

/** Called when the microphone opens: load Whisper while the user talks. */
export function prepareSpeech(purpose: Purpose): void {
  if (whisperFor(purpose))
    void host()
      .whisperPrewarm()
      .catch(() => {});
}

export async function transcribeRecording(rec: Recording, purpose: Purpose): Promise<string> {
  if (whisperFor(purpose) && rec.mimeType === "audio/wav") {
    try {
      return await host().transcribeLocal(rec, purpose);
    } catch (e) {
      const local = toWispError(e);
      const model = useSettings.getState().snapshot?.settings.whisperModel ?? "standard";
      if (local.code === "unsupported") fetchModel(model);
      try {
        return await host().transcribe(rec);
      } catch (e2) {
        const cloud = toWispError(e2);
        // No Gemini key either: say what will make voice work.
        if (cloud.code === "missing_key" && local.code === "unsupported")
          throw {
            ...cloud,
            message:
              "The offline speech model is still downloading. Try again in a minute, or add a Gemini key in Settings → Model.",
          };
        throw cloud;
      }
    }
  }
  return host().transcribe(rec);
}
