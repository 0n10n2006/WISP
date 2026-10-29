import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { Icon, type IconName } from "../../components/Icon";
import { WispMark } from "../../components/WispMark";
import { useOrbState } from "../../hooks/useOrbState";
import { calculate } from "../../lib/calc";
import {
  barResults,
  fileQuery,
  fileResults,
  looksLikeQuestion,
  parseTag,
  TAG_LABELS,
  TAG_PREFIX,
  tagForWord,
  type BarExtras,
  type BarResult,
  type SearchTag,
} from "../../lib/commands";
import { invalidateCatalogue, useCatalogue } from "../../lib/catalogue";
import { host } from "../../services/host";
import { DEFAULT_SCREEN_PROMPT, useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useNotch } from "../../state/notch";
import { ORB_LABELS } from "../../state/orb";
import { useSettings } from "../../state/settings";
import { spokenQuery, useVoice, voicePhase } from "../../state/voice";
import { VoiceMeter } from "./Peek";
import {
  chordLabel,
  type BrowserItem,
  type BrowserTab,
  type CurrencyRates,
  type FileHit,
  type FilePreview,
  type HoldChord,
  type OpenWindow,
  type ProcInfo,
} from "../../services/types";
import { runResult } from "./actions";

export interface CommandBarHandle {
  focus(): void;
  /** Submit the current selection (or a pending screen capture). */
  submit(): void;
}

const NO_STARS: string[] = [];
const NO_FILES: FileHit[] = [];
const NO_PAGES: BrowserItem[] = [];
const NO_TABS: BrowserTab[] = [];
const NO_WINDOWS: OpenWindow[] = [];
const NO_SHORTCUTS: never[] = [];
/** Wait this long after a keystroke before searching files. */
const FILE_SEARCH_DELAY = 60;

function fileIcon(kind: FileHit["kind"]): IconName {
  switch (kind) {
    case "folder":
      return "folder";
    case "image":
      return "image";
    case "video":
      return "play";
    case "audio":
      return "headphones";
    default:
      return "file";
  }
}
const holdTitle = (what: string, chord: HoldChord) =>
  chord === "off" ? what : `${what} (or tap ${chordLabel(chord)})`;
function resultIcon(r: BarResult): IconName {
  if (r.kind === "ask") return "spark";
  if (r.kind === "window") return "external";
  if (r.kind === "file") return fileIcon(r.file.kind);
  if (r.kind === "calc") return "grid";
  if (r.kind === "web") return "search";
  if (r.kind === "tab") return "globe";
  if (r.kind === "history") return r.item.kind === "bookmark" ? "star" : "link";
  if (r.kind === "process") return "cpu";
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
  // A search tag ("t:", "f:"…) shows as a chip; the input holds the rest.
  const [tag, setTag] = useState<SearchTag | null>(null);
  const full = tag ? `${TAG_PREFIX[tag]} ${query}` : query;
  const onInput = (value: string) => {
    const parsed = tag ? null : parseTag(value);
    if (parsed?.tag) {
      setTag(parsed.tag);
      setQuery(parsed.rest);
    } else setQuery(value);
  };
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
  const selected = sel.query === full ? sel.index : 0;

  const awareness = useSettings((s) => s.snapshot?.settings.appAwareness ?? true);
  const holdOn = useSettings((s) => s.snapshot?.settings.holdToTalk ?? true);
  const holdDictate = useSettings(
    (s) => (holdOn ? s.snapshot?.settings.holdDictate : "off") ?? "ctrl+win",
  );
  const holdAsk = useSettings((s) => (holdOn ? s.snapshot?.settings.holdAsk : "off") ?? "win+alt");
  // Windows that are open right now, fetched once per bar open.
  const [openWindows, setWindows] = useState<OpenWindow[]>([]);
  const windows = awareness ? openWindows : NO_WINDOWS;
  useEffect(() => {
    if (!awareness) return;
    let live = true;
    host()
      .openWindows()
      .then((w) => live && setWindows(w))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [awareness]);

  // Files whose names match, from the host's index (a few ms per search).
  const indexOn = useSettings((s) => s.snapshot?.settings.indexFiles ?? true);
  const [found, setFound] = useState<{ query: string; files: FileHit[] }>({
    query: "",
    files: NO_FILES,
  });
  const fileText =
    !indexOn || pending
      ? null
      : tag === "files"
        ? query.trim() || null
        : tag
          ? null
          : fileQuery(query);
  useEffect(() => {
    if (!fileText) return;
    let live = true;
    const t = setTimeout(() => {
      host()
        .filesSearch(fileText, 8)
        .then((files) => live && setFound({ query: full, files }))
        .catch(() => {});
    }, FILE_SEARCH_DELAY);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [fileText, full]);

  // Browser tabs (once per open), history and bookmarks (as you type),
  // running programs (when asked for) and exchange rates (when converting).
  const browserOn = useSettings((s) => s.snapshot?.settings.browserSearch ?? true);
  const webShortcuts = useSettings((s) => s.snapshot?.settings.webShortcuts ?? NO_SHORTCUTS);
  const [openTabs, setTabs] = useState<BrowserTab[]>([]);
  const tabs = browserOn ? openTabs : NO_TABS;
  useEffect(() => {
    if (!browserOn) return;
    let live = true;
    host()
      .browserTabs()
      .then((t) => live && setTabs(t))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [browserOn, tag]);
  const [pages, setPages] = useState<{ query: string; pages: BrowserItem[] }>({
    query: "",
    pages: NO_PAGES,
  });
  const pageText = !browserOn
    ? null
    : tag === "history"
      ? query.trim() || null
      : !tag && query.trim().length >= 3 && !looksLikeQuestion(query)
        ? query.trim()
        : null;
  useEffect(() => {
    if (!pageText) return;
    let live = true;
    const t = setTimeout(() => {
      host()
        .browserSearch(pageText, 6)
        .then((p) => live && setPages({ query: full, pages: p }))
        .catch(() => {});
    }, FILE_SEARCH_DELAY + 20);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [pageText, full]);
  const [procs, setProcs] = useState<ProcInfo[] | null>(null);
  const wantProcs = tag === "processes" || (!tag && query.trim().length >= 3);
  useEffect(() => {
    if (!wantProcs || procs) return;
    let live = true;
    host()
      .processesList()
      .then((p) => live && setProcs(p))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [wantProcs, procs]);
  const [rates, setRates] = useState<CurrencyRates | null>(null);
  const needRates = !rates && /getting today's rates/.test(calculate(full)?.hint ?? "");
  useEffect(() => {
    if (!needRates) return;
    let live = true;
    host()
      .currencyRates()
      .then((r) => live && setRates(r))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [needRates]);
  const extras: BarExtras = useMemo(
    () => ({
      tabs,
      history: pages.query === full ? pages.pages : NO_PAGES,
      processes: procs ?? [],
      shortcuts: webShortcuts,
      rates,
    }),
    [tabs, pages, full, procs, webShortcuts, rates],
  );

  // Asking by voice: the words heard fill the input; when Windows hears a
  // pause, the question runs like pressing Enter.
  const listening = useVoice((v) => v.listening && v.target === "bar");
  const finished = useVoice((v) => v.finished);
  useEffect(() => {
    if (listening) input.current?.focus();
  }, [listening]);
  useEffect(() => {
    const spoken = finished === null ? null : useVoice.getState().takeFinished();
    const q = spoken === null ? "" : spokenQuery(spoken);
    if (!q) return;
    setQuery(q);
    // "Open my resume": look the file up first, so the top result is right.
    const fq = indexOn ? fileQuery(q) : null;
    void (fq ? host().filesSearch(fq, 8) : Promise.resolve(NO_FILES))
      .catch(() => NO_FILES)
      .then((files) => {
        const items = catalogue?.items ?? [];
        const recent = catalogue?.recent ?? [];
        const top = barResults(q, items, starred, recent, windows, files, extras)[0];
        if (top) void runResult(top, { web, think });
      });
  }, [finished, catalogue, starred, windows, web, think, setQuery, indexOn, extras]);
  useEffect(() => () => useVoice.getState().cancel(), []);

  const results = useMemo(() => {
    if (pending) return [];
    const items = catalogue?.items ?? [];
    const recent = catalogue?.recent ?? [];
    const fresh = indexOn && found.query === full;
    const files = fresh ? found.files : NO_FILES;
    const list = barResults(full, items, starred, recent, windows, files, extras);
    // While the next search runs, the last matches stay at the bottom (no flicker).
    const stale = !fresh && fileText ? found.files : NO_FILES;
    return stale.length ? [...list, ...fileResults(stale)] : list;
  }, [full, catalogue, starred, pending, windows, found, indexOn, fileText, extras]);

  // A preview of the selected file, a moment after it's selected.
  const current = results[selected];
  const previewId = current?.kind === "file" ? current.file.id : null;
  const [preview, setPreview] = useState<{ id: number; data: FilePreview } | null>(null);
  useEffect(() => {
    if (previewId === null) return;
    let live = true;
    const t = setTimeout(() => {
      host()
        .filePreview(previewId)
        .then((data) => live && setPreview({ id: previewId, data }))
        .catch(() => {});
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [previewId]);
  const shownPreview =
    current?.kind === "file" && preview?.id === current.file.id ? preview.data : null;

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
    if (listening) {
      // Holding the voice keys (Win+Alt) repeats them here: that's not typing.
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        useVoice.getState().cancel();
        return;
      }
      if (e.key === "Enter") {
        // Ask what was said so far, now.
        e.preventDefault();
        useVoice.getState().stop();
        return;
      }
      const typing =
        !e.repeat &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.metaKey &&
        (e.key.length === 1 || e.key === "Backspace" || e.key === "Delete");
      if (!typing) return;
      // Typing while Wisp listens means editing: stop listening, keep the words.
      useVoice.getState().cancel();
    }
    if (e.key === "Backspace" && tag && !query) {
      // Backspace on an empty search removes the tag.
      e.preventDefault();
      setTag(null);
    } else if (e.key === "ArrowDown" && results.length) {
      e.preventDefault();
      setSel({ query: full, index: (selected + 1) % results.length });
    } else if (e.key === "ArrowUp" && results.length) {
      e.preventDefault();
      setSel({ query: full, index: (selected - 1 + results.length) % results.length });
    } else if (e.key === "Enter") {
      e.preventDefault();
      const r = results[selected];
      // Shift+Enter asks with "think harder" for this one question.
      if (e.shiftKey && r?.kind === "ask") void runResult(r, { web, think: true });
      // Ctrl+Enter: show a file in its folder, or end a program.
      else if ((e.ctrlKey || e.metaKey) && (r?.kind === "file" || r?.kind === "process"))
        void runResult(r, { web, think, alt: true });
      else submit(r);
    } else if (e.key === "Tab" && !tag && tagForWord(query) && !e.shiftKey) {
      // "tabs" + Tab searches tabs only.
      e.preventDefault();
      setTag(tagForWord(query));
      setQuery("");
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
        {tag && (
          <button
            className="cmd-tag"
            onClick={() => {
              setTag(null);
              input.current?.focus();
            }}
            aria-label={`Searching ${TAG_LABELS[tag]}. Remove`}
            title="Remove (Backspace)"
          >
            {TAG_LABELS[tag]}
            <Icon name="close" size={12} />
          </button>
        )}
        <input
          ref={input}
          className="cmd-input"
          value={query}
          onChange={(e) => onInput(e.target.value)}
          onKeyDown={onKeyDown}
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
      {listening && <VoiceHint />}
      {results.length > 0 && (
        <div className="cmd-body">
          <ul className="cmd-results" id={listId} role="listbox" aria-label="Results">
            {results.map((r, i) => (
              <li
                key={`${r.kind}-${r.id}`}
                id={`bar-r-${i}`}
                role="option"
                aria-selected={i === selected}
                className={`cmd-result ${i === selected ? "selected" : ""}`}
                onMouseMove={() => i !== selected && setSel({ query: full, index: i })}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => submit(r)}
              >
                <span className={`cmd-result-icon kind-${r.kind}`}>
                  <Icon name={resultIcon(r)} size={16} />
                </span>
                <span className="cmd-result-title">{r.title}</span>
                {r.kind === "file" && i === selected && (
                  <button
                    className="icon-btn icon-btn-small cmd-result-reveal"
                    aria-label={`Show ${r.title} in its folder`}
                    title="Show in folder (Ctrl+Enter)"
                    onClick={(e) => {
                      e.stopPropagation();
                      void runResult(r, { web, think, alt: true });
                    }}
                  >
                    <Icon name="folder" size={14} />
                  </button>
                )}
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
          {current?.kind === "file" && (
            <FilePreviewPane file={current.file} preview={shownPreview} />
          )}
        </div>
      )}
    </div>
  );
});

function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** What the selected file looks like, beside the results. */
function FilePreviewPane({ file, preview }: { file: FileHit; preview: FilePreview | null }) {
  const date = new Date(file.modified).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
  return (
    <aside className="cmd-preview" aria-label={`Preview of ${file.name}`}>
      {preview?.kind === "image" && preview.image && (
        <img className="cmd-preview-img" src={preview.image} alt="" />
      )}
      {preview?.kind === "text" && <pre className="cmd-preview-text">{preview.text}</pre>}
      {preview?.kind === "folder" && (
        <ul className="cmd-preview-list">
          {(preview.entries ?? []).map((e) => (
            <li key={e}>{e}</li>
          ))}
          {!preview.entries?.length && <li className="muted">Empty folder</li>}
        </ul>
      )}
      {!preview && <div className="cmd-preview-placeholder" aria-hidden />}
      <p className="cmd-preview-name">{file.name}</p>
      <p className="cmd-preview-meta">
        {file.folder || "Folder"}
        <br />
        {file.kind === "folder" ? date : `${formatSize(file.size)} · ${date}`}
      </p>
    </aside>
  );
}

/** While asking by voice: are you being heard, and how to finish. */
function VoiceHint() {
  const live = useVoice((v) => v.live);
  const heardAt = useVoice((v) => v.heardAt);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 200);
    return () => clearInterval(t);
  }, []);
  const phase = voicePhase({ listening: true, live, heardAt }, now);
  return (
    <p className={`voice-hint voice-${phase}`} role="status">
      <VoiceMeter phase={phase} />
      <span>
        {phase === "waiting"
          ? "Listening — start talking. Tap the keys or click the mic to stop."
          : phase === "hearing"
            ? "Hearing you…"
            : "Pause a moment and Wisp asks. Type to edit instead."}
      </span>
    </p>
  );
}
