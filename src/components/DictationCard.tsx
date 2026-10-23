import { useEffect, useRef, useState } from "react";
import { presentError } from "../services/errors";
import { host } from "../services/host";
import { useConversation } from "../state/conversation";
import { useDictation } from "../state/dictation";
import { Icon } from "./Icon";

function useElapsed(active: boolean, since: number): string {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(t);
  }, [active]);
  const s = Math.max(0, Math.floor((now - since) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Dictation flow inside the island: listening → transcribing → review. */
export function DictationCard() {
  const d = useDictation();
  const elapsed = useElapsed(d.phase === "listening", d.startedAt);
  const primary = useRef<HTMLButtonElement>(null);
  const editor = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (d.phase === "listening") primary.current?.focus();
    if (d.phase === "review") editor.current?.focus();
  }, [d.phase]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      if (d.phase === "listening") void d.stop();
      else if (d.phase !== "review" || !d.transcript.trim()) d.cancel();
    }
  };

  return (
    <section className="dictation" aria-label="Dictation" onKeyDown={onKeyDown}>
      {(d.phase === "starting" || d.phase === "listening") && (
        <div className="dictation-live">
          <div
            className="mic-meter"
            role="meter"
            aria-label="Microphone level"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(d.level * 100)}
          >
            <Icon name="mic" size={28} />
            <span
              className="mic-ring"
              style={{ transform: `scale(${1 + d.level * 0.6})` }}
              aria-hidden
            />
          </div>
          <p className="dictation-title" aria-live="assertive">
            {d.phase === "starting" ? "Starting microphone…" : "Listening…"}
          </p>
          {d.phase === "listening" && <p className="dictation-time">{elapsed}</p>}
          {d.live ? (
            <p className="live-words" aria-live="polite">
              {d.live}
            </p>
          ) : (
            <p className="dictation-note">
              {d.instant
                ? "Words appear as Windows recognises them; Gemini steps in if it can't."
                : "Your audio is sent to Gemini for transcription, then discarded."}
            </p>
          )}
          <div className="row-center">
            <button
              ref={primary}
              className="btn btn-primary"
              onClick={() => void d.stop()}
              disabled={d.phase !== "listening"}
            >
              <Icon name="stop" size={14} /> Stop
            </button>
            <button className="btn btn-ghost" onClick={d.cancel}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {d.phase === "transcribing" && (
        <div className="dictation-live">
          <p className="dictation-title" aria-live="polite">
            Transcribing…
          </p>
          <div className="row-center">
            <button className="btn btn-ghost" onClick={d.cancel}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {d.phase === "review" && (
        <div className="dictation-review">
          {d.heard ? (
            <>
              <label htmlFor="transcript" className="dictation-label">
                Transcript — edit before inserting
              </label>
              <textarea
                id="transcript"
                ref={editor}
                value={d.transcript}
                onChange={(e) => d.setTranscript(e.target.value)}
                rows={5}
              />
            </>
          ) : (
            <p className="dictation-title">No speech detected.</p>
          )}
          {d.error && (
            <p className="field-error" role="alert">
              <Icon name="warning" size={14} /> {d.error.message}
            </p>
          )}
          <div className="row-start">
            {d.transcript && d.target.available && (
              <button className="btn btn-primary" onClick={() => void d.insert()}>
                Insert
                {d.target.title
                  ? ` into ${d.target.title.length > 24 ? d.target.title.slice(0, 24) + "…" : d.target.title}`
                  : ""}
              </button>
            )}
            {d.transcript && (
              <button
                className={`btn ${d.target.available ? "" : "btn-primary"}`}
                onClick={() => void d.copy()}
              >
                <Icon name="copy" size={14} /> Copy
              </button>
            )}
            {d.transcript && (
              <button
                className="btn btn-ghost"
                onClick={() => {
                  useConversation.getState().setDraft(d.transcript);
                  d.reset();
                }}
              >
                Ask Wisp
              </button>
            )}
            {!d.heard && (
              <button className="btn btn-primary" onClick={() => void d.start()}>
                <Icon name="mic" size={14} /> Try again
              </button>
            )}
            <button className="btn btn-ghost" onClick={d.cancel}>
              Discard
            </button>
          </div>
        </div>
      )}

      {d.phase === "error" && d.error && (
        <div className="dictation-review" role="alert">
          <p className="dictation-title">{presentError(d.error).title}</p>
          <p className="setting-desc">{d.error.message}</p>
          <div className="row-start">
            <button className="btn btn-primary" onClick={() => void d.start()}>
              Try again
            </button>
            {d.error.code === "missing_key" || d.error.code === "invalid_key" ? (
              <button className="btn" onClick={() => void host().openSettings("model")}>
                Open settings
              </button>
            ) : null}
            <button className="btn btn-ghost" onClick={d.cancel}>
              Close
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
