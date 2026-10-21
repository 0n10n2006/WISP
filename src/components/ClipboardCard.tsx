import { useEffect, useRef, useState } from "react";
import { useConversation } from "../state/conversation";
import { MAX_CLIPBOARD_CHARS, useClipboardAssist } from "../state/assist";
import { Icon } from "./Icon";

const PRESETS = [
  { id: "summarize", label: "Summarize", prompt: "Summarize the following text concisely." },
  {
    id: "rewrite",
    label: "Rewrite",
    prompt:
      "Rewrite the following text to be clearer and more polished. Keep its meaning and tone. Return only the rewritten text.",
  },
  { id: "explain", label: "Explain", prompt: "Explain the following text in simple terms." },
] as const;

/** Explicit, one-shot clipboard help. The clipboard is read once when opened. */
export function ClipboardCard() {
  const { loading, text, error, close } = useClipboardAssist();
  const ask = useConversation((s) => s.ask);
  const [question, setQuestion] = useState("");
  const first = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!loading) first.current?.focus();
  }, [loading]);

  const clipped =
    text && text.length > MAX_CLIPBOARD_CHARS ? text.slice(0, MAX_CLIPBOARD_CHARS) : text;

  const send = (instruction: string, label: string) => {
    if (!clipped) return;
    close();
    void ask(`${instruction}\n\n---\n${clipped}`, {
      contextKind: "clipboard",
      displayText: `${label} — clipboard text (${clipped.length.toLocaleString()} characters)`,
    });
  };

  return (
    <section
      className="clipboard-card"
      aria-label="Clipboard help"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          close();
        }
      }}
    >
      <header className="row-between card-head">
        <strong>
          <Icon name="clipboard" size={16} /> Clipboard help
        </strong>
        <button className="icon-btn" onClick={close} aria-label="Close clipboard help">
          <Icon name="close" size={14} />
        </button>
      </header>
      {loading && <p className="muted">Reading clipboard…</p>}
      {error && (
        <p className="field-error" role="alert">
          {error.message}
        </p>
      )}
      {!loading && !error && !text && (
        <p className="setting-desc" role="status">
          Your clipboard doesn't contain text. Copy some text and try again.
        </p>
      )}
      {clipped && (
        <>
          <blockquote className="clip-preview" tabIndex={0} aria-label="Clipboard text">
            {clipped.length > 600 ? `${clipped.slice(0, 600)}…` : clipped}
          </blockquote>
          <p className="setting-desc">
            {clipped.length.toLocaleString()} characters will be sent to Google Gemini when you
            choose an action.
            {text &&
              text.length > MAX_CLIPBOARD_CHARS &&
              " Only the first 20,000 characters are used."}
          </p>
          <div className="row-start">
            {PRESETS.map((p, i) => (
              <button
                key={p.id}
                ref={i === 0 ? first : undefined}
                className={`btn ${i === 0 ? "btn-primary" : ""}`}
                onClick={() => send(p.prompt, p.label)}
              >
                {p.label}
              </button>
            ))}
          </div>
          <form
            className="input-row clip-question"
            onSubmit={(e) => {
              e.preventDefault();
              if (question.trim()) send(question.trim(), question.trim());
            }}
          >
            <label htmlFor="clip-q" className="sr-only">
              Ask about the clipboard text
            </label>
            <input
              id="clip-q"
              type="text"
              value={question}
              placeholder="Or ask something about it…"
              onChange={(e) => setQuestion(e.target.value)}
            />
            <button className="btn" disabled={!question.trim()}>
              Ask
            </button>
          </form>
        </>
      )}
    </section>
  );
}
