import { useEffect, useRef } from "react";
import { host } from "../services/host";
import type { Message } from "../state/conversation";
import { speak, speechAvailable } from "../services/speech";
import { useSettings } from "../state/settings";
import { toast } from "../state/toast";
import { ActionNotes } from "./ActionNotes";
import { Icon } from "./Icon";
import { Markdown } from "./Markdown";

function openLink(url: string) {
  host()
    .openExternal(url)
    .catch(() => toast("Couldn't open that link", "warning"));
}

async function copy(text: string) {
  try {
    await host().copyText(text);
    toast("Copied", "success");
  } catch {
    toast("Couldn't copy", "warning");
  }
}

function readAloud(text: string) {
  const s = useSettings.getState().snapshot?.settings;
  void speak(text, { voice: s?.ttsVoice || undefined, rate: s?.ttsRate ?? 1 });
}

export function MessageList({ messages }: { messages: Message[] }) {
  const end = useRef<HTMLDivElement>(null);
  const last = messages[messages.length - 1];
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [messages.length, last?.text]);

  return (
    <div
      className="messages"
      role="log"
      aria-label="Conversation"
      aria-live="polite"
      aria-relevant="additions"
    >
      {messages.map((m) =>
        m.role === "user" ? (
          <div key={m.id} className="msg msg-user">
            {m.imagePreview && (
              <img className="msg-image" src={m.imagePreview} alt="Selected screen region" />
            )}
            {m.text && <div className="bubble">{m.text}</div>}
          </div>
        ) : (
          <div key={m.id} className="msg msg-model" aria-busy={m.status === "streaming"}>
            {m.actions?.length ? <ActionNotes notes={m.actions} /> : null}
            {m.text ? (
              <Markdown text={m.text} onLink={openLink} onCopy={(code) => void copy(code)} />
            ) : m.actions?.some((a) => a.confirmId) ? null : (
              <p className="msg-pending">Thinking…</p>
            )}
            {m.status === "streaming" && m.text && <span className="cursor" aria-hidden />}
            {m.status !== "streaming" && m.text && (
              <div className="msg-actions">
                <button className="btn btn-small btn-ghost" onClick={() => void copy(m.text)}>
                  <Icon name="copy" size={14} /> Copy
                </button>
                {speechAvailable() && (
                  <button
                    className="btn btn-small btn-ghost"
                    onClick={() => void readAloud(m.text)}
                  >
                    <Icon name="speaker" size={14} /> Read aloud
                  </button>
                )}
                {m.status === "stopped" && <span className="tag">Stopped</span>}
                {m.status === "failed" && <span className="tag tag-warning">Incomplete</span>}
              </div>
            )}
          </div>
        ),
      )}
      <div ref={end} />
    </div>
  );
}
