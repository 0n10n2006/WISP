import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { ClipEntry } from "../../services/types";
import { useConversation } from "../../state/conversation";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { timeAgo } from "./util";

type Filter = "all" | "text" | "link";

function Enable() {
  const update = useSettings((s) => s.update);
  return (
    <div className="empty-card">
      <Icon name="clipboard" size={26} />
      <p>
        <strong>Clipboard history is off</strong>
      </p>
      <p className="muted">
        When on, Wisp keeps the last 100 things you copy (text and links) on this PC so you can
        paste them again. Copies that password managers mark as private, and text that looks like a
        password or key, are never stored. Nothing is sent anywhere.
      </p>
      <button
        className="btn btn-primary btn-small"
        onClick={() => void update({ clipboardHistory: true })}
      >
        Turn on clipboard history
      </button>
    </div>
  );
}

export default function ClipboardTab() {
  const enabled = useSettings((s) => s.snapshot?.settings.clipboardHistory ?? false);
  const [clips, setClips] = useState<ClipEntry[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  useEffect(() => {
    if (!enabled) return;
    const load = () =>
      void host()
        .clipsList(filter === "all" ? undefined : filter)
        .then(setClips)
        .catch(() => {});
    load();
    return host().onDataChanged((s) => (s === "clipboard" || s === "everything") && load());
  }, [enabled, filter]);
  if (!enabled) return <Enable />;
  const q = search.trim().toLowerCase();
  const shown = q ? clips.filter((c) => c.text.toLowerCase().includes(q)) : clips;
  const run = (fn: () => Promise<unknown>, ok?: string) =>
    void fn()
      .then(() => ok && toast(ok, "success"))
      .catch((e) => toast(toWispError(e).message, "warning"));
  return (
    <div className="clips">
      <div className="row">
        <div className="segmented" role="radiogroup" aria-label="Show">
          {(["all", "text", "link"] as Filter[]).map((f) => (
            <button
              key={f}
              role="radio"
              aria-checked={filter === f}
              className={filter === f ? "on" : ""}
              onClick={() => setFilter(f)}
            >
              {f === "all" ? "All" : f === "text" ? "Text" : "Links"}
            </button>
          ))}
        </div>
        <input
          className="input"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search clipboard"
          aria-label="Search clipboard"
        />
        <button
          className="btn btn-small btn-ghost"
          onClick={() => run(() => host().clipsClear(false), "Cleared")}
        >
          Clear
        </button>
        <button
          className="btn btn-small btn-ghost"
          title="Turn clipboard history off"
          onClick={() => void useSettings.getState().update({ clipboardHistory: false })}
        >
          Turn off
        </button>
      </div>
      {shown.length === 0 ? (
        <p className="muted pad">{q ? "No matches." : "Copy something and it will appear here."}</p>
      ) : (
        <ul className="clip-list">
          {shown.map((c) => (
            <li key={c.id} className={`clip ${c.pinned ? "pinned" : ""}`}>
              <button
                className="clip-main"
                onClick={() => run(() => host().clipCopy(c.id), "Copied")}
                title="Copy again"
              >
                <Icon name={c.kind === "link" ? "link" : "copy"} size={14} />
                <span className="clip-text">{c.text}</span>
                <span className="clip-time muted">{timeAgo(c.createdAt)}</span>
              </button>
              <div className="clip-actions">
                <button
                  className="icon-btn icon-btn-tiny"
                  aria-label="Ask Wisp about this"
                  title="Ask Wisp about this"
                  onClick={() =>
                    void useConversation.getState().ask(`Help me with this:\n\n---\n${c.text}`, {
                      contextKind: "clipboard",
                      displayText: `About a clipboard item (${c.text.length.toLocaleString()} characters)`,
                    })
                  }
                >
                  <Icon name="spark" size={14} />
                </button>
                <button
                  className={`icon-btn icon-btn-tiny ${c.pinned ? "icon-btn-on" : ""}`}
                  aria-label={c.pinned ? "Unpin" : "Pin"}
                  aria-pressed={c.pinned}
                  onClick={() => run(() => host().clipPin(c.id, !c.pinned))}
                >
                  <Icon name="pin" size={14} />
                </button>
                <button
                  className="icon-btn icon-btn-tiny"
                  aria-label="Delete"
                  onClick={() => run(() => host().clipDelete(c.id))}
                >
                  <Icon name="trash" size={14} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
