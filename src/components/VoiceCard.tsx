import { useEffect, useRef } from "react";
import { presentError } from "../services/errors";
import { host } from "../services/host";
import { useVoice } from "../state/voice";
import { Icon } from "./Icon";

/** Push-to-talk conversation: listening → transcribing, then back to the chat. */
export function VoiceCard() {
  const v = useVoice();
  const primary = useRef<HTMLButtonElement>(null);
  useEffect(() => primary.current?.focus(), [v.phase]);

  return (
    <section
      className="dictation"
      aria-label="Voice conversation"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          v.cancel();
        }
      }}
    >
      {v.phase === "listening" && (
        <div className="dictation-live">
          <div
            className="mic-meter"
            role="meter"
            aria-label="Microphone level"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(v.level * 100)}
          >
            <Icon name="talk" size={28} />
            <span
              className="mic-ring"
              style={{ transform: `scale(${1 + v.level * 0.6})` }}
              aria-hidden
            />
          </div>
          <p className="dictation-title" aria-live="assertive">
            Listening…
          </p>
          <p className="dictation-note">
            Press the voice shortcut again or Done when you've finished.
          </p>
          <div className="row-center">
            <button ref={primary} className="btn btn-primary" onClick={() => void v.finish()}>
              Done
            </button>
            <button className="btn btn-ghost" onClick={v.cancel}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {v.phase === "transcribing" && (
        <div className="dictation-live">
          <p className="dictation-title" aria-live="polite">
            Thinking…
          </p>
          <div className="row-center">
            <button ref={primary} className="btn btn-ghost" onClick={v.cancel}>
              Cancel
            </button>
          </div>
        </div>
      )}
      {v.phase === "error" && v.error && (
        <div className="dictation-review" role="alert">
          <p className="dictation-title">{presentError(v.error).title}</p>
          <p className="setting-desc">{v.error.message}</p>
          <div className="row-start">
            <button ref={primary} className="btn btn-primary" onClick={() => void v.start()}>
              Try again
            </button>
            {(v.error.code === "permission_denied" || v.error.code === "missing_key") && (
              <button
                className="btn"
                onClick={() =>
                  void host().openSettings(v.error?.code === "missing_key" ? "model" : "voice")
                }
              >
                Open settings
              </button>
            )}
            <button className="btn btn-ghost" onClick={v.cancel}>
              Close
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
