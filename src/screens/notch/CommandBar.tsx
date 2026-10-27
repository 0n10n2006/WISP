import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { Icon, type IconName } from "../../components/Icon";
import { WispMark } from "../../components/WispMark";
import { useOrbState } from "../../hooks/useOrbState";
import { barResults, type BarResult } from "../../lib/commands";
import { invalidateCatalogue, useCatalogue } from "../../lib/catalogue";
import { host } from "../../services/host";
import { DEFAULT_SCREEN_PROMPT, useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useNotch } from "../../state/notch";
import { ORB_LABELS } from "../../state/orb";
import { useSettings } from "../../state/settings";
import { spokenQuery, useVoice, VOICE_SEND_MS } from "../../state/voice";
import { chordLabel, type HoldChord, type OpenWindow } from "../../services/types";
import { runResult } from "./actions";

export interface CommandBarHandle {
  focus(): void;
  /** Submit the current selection (or a pending screen capture). */
  submit(): void;
}

const NO_STARS: string[] = [];
const holdTitle = (what: string, chord: HoldChord) =>
  chord === "off" ? what : `${what} (or tap ${chordLabel(chord)})`;
function resultIcon(r: BarResult): IconName {
  if (r.kind === "ask") return "spark";
  if (r.kind === "window") return "external";
  if (r.kind === "app")
    return r.item.kind === "game" ? "play" : r.item.kind === "system" ? "settings" : "grid";
  switch (r.command.type) {
    case "power":
      return r.command.action === "lock"
        ? "lock"
        : r.command.action === "sleep"
          ? "sleep"
          : "power";
    case "timer":
    case "stopTimer":
      return "timer";
    case "remind":
      return "calendar";
    case "volume":
      return "volume";
    case "mute":
      return r.command.muted ? "mute" : "volume";
    case "media":
      return r.command.command === "next"
        ? "next"
        : r.command.command === "previous"
          ? "prev"
          : "play";
    case "darkMode":
      return r.command.dark ? "moon" : "sun";
    case "radio":
      return r.command.kind === "wifi" ? "wifi" : "bluetooth";
    case "wallpaper":
      return "shuffle";
    case "winSetting":
      switch (r.command.change.setting) {
        case "nightLight":
          return "nightlight";
        case "doNotDisturb":
          return "bell-off";
        case "brightness":
          return "brightness";
        case "powerMode":
          return "gauge";
        case "micMute":
          return "mic-off";
        case "outputDevice":
          return "headphones";
        case "desktopIcons":
          return "desktop";
        case "transparency":
          return "layers";
        case "accentColor":
          return "sun";
        default:
          return "taskbar";
      }
    case "showMe":
      return "target";
    case "research":
    case "replica":
      return "bot";
    case "selectArea":
      return "lasso";
    case "dictate":
      return "mic";
    case "tuck":
      return "chevron-up";
    default:
      return "chevron-right";
  }
}

export const CommandBar = forwardRef<CommandBarHandle, { onClose(): void }>(function CommandBar(
  { onClose },
  ref,
) {
  const input = useRef<HTMLInputElement>(null);
  const submitRef = useRef<() => void>(() => {});
  useImperativeHandle(ref, () => ({
    focus: () => input.current?.focus(),
    submit: () => submitRef.current(),
  }));
  const query = useNotch((n) => n.query);
  const setQuery = useNotch((n) => n.setQuery);
  const web = useNotch((n) => n.web);
  const think = useNotch((n) => n.think);
  const setWeb = useNotch((n) => n.setWeb);
  const setThink = useNotch((n) => n.setThink);
  const requestActive = useConversation((c) => c.requestActive);
  const cancel = useConversation((c) => c.cancel);
  const pending = useCapture((c) => c.pending);
  const starred = useSettings((s) => s.snapshot?.settings.starredApps ?? NO_STARS);
  const shortcuts = useSettings((s) => s.snapshot?.settings.globalShortcuts);
  const memoryOn = useSettings((s) => s.snapshot?.settings.useMemory ?? false);
  const memoryForChat = useConversation((c) => c.memoryForChat);
  const setMemoryForChat = useConversation((c) => c.setMemoryForChat);
  const state = useOrbState();
  const catalogue = useCatalogue();
  // The highlighted row resets whenever the query changes.
  const [sel, setSel] = useState({ query: "", index: 0 });
  const selected = sel.query === query ? sel.index : 0;

  const awareness = useSettings((s) => s.snapshot?.settings.appAwareness ?? true);
  const holdOn = useSettings((s) => s.snapshot?.settings.holdToTalk ?? true);
  const holdDictate = useSettings(
    (s) => (holdOn ? s.snapshot?.settings.holdDictate : "off") ?? "ctrl+win",
  );
  const holdAsk = useSettings((s) => (holdOn ? s.snapshot?.settings.holdAsk : "off") ?? "win+alt");
  // Windows that are open right now, fetched once per bar open.
  const [windows, setWindows] = useState<OpenWindow[]>([]);
  useEffect(() => {
    if (!awareness) return setWindows([]);
    let live = true;
    host()
      .openWindows()
      .then((w) => live && setWindows(w))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [awareness]);

  // Asking by voice: Windows types the words in; once they stop for a
  // moment, the question is asked (like pressing Enter).
  const listening = useVoice((v) => v.listening);
  const [composing, setComposing] = useState(false);
  // Windows types into whatever has focus: make that the input.
  useEffect(() => {
    if (listening) input.current?.focus();
  }, [listening]);
  useEffect(() => {
    if (!listening || composing || !spokenQuery(query)) return;
    const t = setTimeout(() => {
      const q = spokenQuery(useNotch.getState().query);
      useVoice.getState().stop();
      if (!q) return;
      setQuery(q);
      const top = barResults(
        q,
        catalogue?.items ?? [],
        starred,
        catalogue?.recent ?? [],
        windows,
      )[0];
      if (top) void runResult(top, { web, think });
    }, VOICE_SEND_MS);
    return () => clearTimeout(t);
  }, [listening, composing, query, catalogue, starred, windows, web, think, setQuery]);
  useEffect(() => () => useVoice.getState().stop(), []);

  const results = useMemo(
    () =>
      pending
        ? []
        : barResults(query, catalogue?.items ?? [], starred, catalogue?.recent ?? [], windows),
    [query, catalogue, starred, pending, windows],
  );

  const submit = (r: BarResult | undefined) => {
    if (pending) {
      const prompt = query.trim() || DEFAULT_SCREEN_PROMPT;
      useCapture.getState().setPending(null);
      setQuery("");
      void useConversation.getState().ask(prompt, {
        contextKind: "screen",
        attachments: [{ mimeType: pending.mimeType, dataBase64: pending.dataBase64 }],
        imagePreview: pending.dataUrl,
        web,
        think,
      });
      return;
    }
    if (!r) return;
    if (r.kind === "app") invalidateCatalogue();
    void runResult(r, { web, think });
  };

  useEffect(() => {
    submitRef.current = () => submit(results[selected]);
  });

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // Keys the user types (not Windows' voice typing) mean they're editing.
    if (listening && !e.nativeEvent.isComposing && e.key !== "Process" && e.key !== "Enter") {
      useVoice.getState().stop();
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        return;
      }
    }
    if (e.key === "Enter") useVoice.getState().stop();
    if (e.key === "ArrowDown" && results.length) {
      e.preventDefault();
      setSel({ query, index: (selected + 1) % results.length });
    } else if (e.key === "ArrowUp" && results.length) {
      e.preventDefault();
      setSel({ query, index: (selected - 1 + results.length) % results.length });
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results[selected];
      // Shift+Enter asks with "think harder" for this one question.
      if (e.shiftKey && r?.kind === "ask") void runResult(r, { web, think: true });
      else submit(r);
    } else if (e.key === "Tab" && query.trim() && !e.shiftKey) {
      // Tab adds (or removes) the web for this question.
      e.preventDefault();
      setWeb(!web);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (requestActive) cancel();
      else if (pending) useCapture.getState().setPending(null);
      else onClose();
    }
  };

  const listId = "bar-results";
  const activeId = results[selected] ? `bar-r-${selected}` : undefined;
  const placeholder = pending
    ? "Ask about this area (optional)"
    : listening
      ? "Speak now…"
      : "Ask or type a command…";

  return (
    <div className="cmd">
      <div className="cmd-row">
        <span className="cmd-mark" title={ORB_LABELS[state]}>
          <WispMark state={state} size={22} />
        </span>
        <input
          ref={input}
          className="cmd-input"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          onCompositionStart={() => setComposing(true)}
          onCompositionEnd={() => setComposing(false)}
          placeholder={placeholder}
          aria-label="Ask Wisp or search"
          role="combobox"
          aria-expanded={results.length > 0}
          aria-controls={listId}
          aria-activedescendant={activeId}
          aria-autocomplete="list"
          spellCheck={false}
          autoComplete="off"
          maxLength={8000}
        />
        <span className="sr-only" aria-live="polite">
          {ORB_LABELS[state]}
        </span>
        {requestActive ? (
          <button
            className="icon-btn stop"
            onClick={cancel}
            aria-label="Stop answer"
            title="Stop (Esc)"
          >
            <Icon name="stop" size={16} />
          </button>
        ) : (
          <div className="cmd-tools" role="toolbar" aria-label="Options">
            <button
              className={`chip-toggle ${web ? "on" : ""}`}
              aria-pressed={web}
              onClick={() => setWeb(!web)}
              title="Search the web for this answer (Tab)"
            >
              <Icon name="globe" size={15} />
              <span>Web</span>
            </button>
            <button
              className={`chip-toggle ${think ? "on" : ""}`}
              aria-pressed={think}
              onClick={() => setThink(!think)}
              title="Think harder: slower, better for tricky questions (Shift+Enter)"
            >
              <Icon name="spark" size={15} />
              <span>Think</span>
            </button>
            {memoryOn && (
              <button
                className={`icon-btn icon-btn-small ${memoryForChat ? "icon-btn-on" : ""}`}
                onClick={() => setMemoryForChat(!memoryForChat)}
                aria-label="Use memory in this conversation"
                aria-pressed={memoryForChat}
                title={
                  memoryForChat
                    ? "Memory is used in this conversation. Click to turn off."
                    : "Memory is off for this conversation."
                }
              >
                <Icon name="memory" size={16} />
              </button>
            )}
            <span className="cmd-sep" aria-hidden />
            <button
              className="icon-btn icon-btn-small"
              onClick={() => void host().startCapture()}
              aria-label="Select an area to ask about"
              title={`Circle or select an area${shortcuts?.selectToAsk ? ` (${shortcuts.selectToAsk})` : ""}`}
            >
              <Icon name="lasso" size={17} />
            </button>
            <button
              className="icon-btn icon-btn-small"
              onClick={() => void useVoice.getState().dictate()}
              aria-label="Dictate"
              title={holdTitle("Dictate", holdDictate)}
            >
              <Icon name="mic" size={17} />
            </button>
            <button
              className="icon-btn icon-btn-small"
              onClick={() => void useVoice.getState().ask()}
              aria-label="Ask by voice"
              title={holdTitle("Ask by voice", holdAsk)}
            >
              <Icon name="talk" size={17} />
            </button>
            <button
              className="icon-btn icon-btn-small"
              onClick={() => void host().openSettings()}
              aria-label="Settings"
              title="Settings"
            >
              <Icon name="settings" size={17} />
            </button>
            <button
              className="icon-btn icon-btn-small"
              onClick={onClose}
              aria-label="Close"
              title="Close (Esc)"
            >
              <Icon name="chevron-up" size={17} />
            </button>
          </div>
        )}
      </div>
      {listening && (
        <p className="voice-hint" role="status">
          <Icon name="mic" size={14} />
          {spokenQuery(query) ? (
            <>
              <span>Asking when you stop… Enter asks now, typing lets you edit.</span>
              <span key={query} className="voice-countdown" aria-hidden />
            </>
          ) : (
            <span>Listening — Windows voice typing writes what you say here.</span>
          )}
        </p>
      )}
      {results.length > 0 && (
        <ul className="cmd-results" id={listId} role="listbox" aria-label="Results">
          {results.map((r, i) => (
            <li
              key={`${r.kind}-${r.id}`}
              id={`bar-r-${i}`}
              role="option"
              aria-selected={i === selected}
              className={`cmd-result ${i === selected ? "selected" : ""}`}
              onMouseMove={() => i !== selected && setSel({ query, index: i })}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => submit(r)}
            >
              <span className={`cmd-result-icon kind-${r.kind}`}>
                <Icon name={resultIcon(r)} size={16} />
              </span>
              <span className="cmd-result-title">{r.title}</span>
              <span className="cmd-result-hint">
                {r.kind === "ask" ? (
                  <>
                    {web ? "Ask with web" : think ? "Ask, thinking harder" : "Ask Wisp"}
                    {i === selected && <kbd>Enter</kbd>}
                  </>
                ) : (
                  <>
                    {r.hint}
                    {i === selected && <kbd>Enter</kbd>}
                  </>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
});
