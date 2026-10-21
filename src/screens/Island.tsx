import { useCallback, useEffect, useRef, useState, type PointerEvent } from "react";
import { DictationCard } from "../components/DictationCard";
import { ErrorBanner } from "../components/ErrorBanner";
import { Icon } from "../components/Icon";
import { MessageList } from "../components/MessageList";
import { Orb } from "../components/Orb";
import { PromptBox, type PromptBoxHandle } from "../components/PromptBox";
import { ToastHost } from "../components/Toast";
import { useOnline } from "../hooks/useOnline";
import { useOrbState } from "../hooks/useOrbState";
import { host } from "../services/host";
import type { IslandLayout, ShortcutAction } from "../services/types";
import { DEFAULT_SCREEN_PROMPT, useCapture } from "../state/capture";
import { useConversation } from "../state/conversation";
import { useDictation } from "../state/dictation";
import { ORB_LABELS } from "../state/orb";
import { useSettings } from "../state/settings";
import { toast } from "../state/toast";

const DRAG_THRESHOLD = 4;

function settingsTabFor(code: string): string {
  if (code === "permission_denied") return "privacy";
  return "model";
}

/** Feature handlers registered by later milestones (screen ask, dictation, …). */
export type ShortcutHandler = (action: ShortcutAction) => boolean;
const shortcutHandlers: ShortcutHandler[] = [];
export function registerShortcutHandler(h: ShortcutHandler): () => void {
  shortcutHandlers.push(h);
  return () => {
    const i = shortcutHandlers.indexOf(h);
    if (i >= 0) shortcutHandlers.splice(i, 1);
  };
}

function CollapsedOrb({ onActivate }: { onActivate(): void }) {
  const state = useOrbState();
  const start = useRef<{ x: number; y: number } | null>(null);
  const dragged = useRef(false);

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    start.current = { x: e.clientX, y: e.clientY };
    dragged.current = false;
  };
  const onPointerMove = (e: PointerEvent) => {
    const s = start.current;
    if (!s || dragged.current || (e.buttons & 1) === 0) return;
    if (Math.hypot(e.clientX - s.x, e.clientY - s.y) > DRAG_THRESHOLD) {
      dragged.current = true;
      void host().startDragging();
    }
  };
  const onClick = () => {
    start.current = null;
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    onActivate();
  };

  return (
    <div className="island-collapsed">
      <button
        className="orb-button"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onClick={onClick}
        aria-label={`Open Wisp — ${ORB_LABELS[state]}`}
        title={`Wisp — ${ORB_LABELS[state]}. Click to open, drag to move.`}
      >
        <Orb state={state} size={60} decorative />
      </button>
    </div>
  );
}

function SetupCard() {
  return (
    <div className="setup-card">
      <Icon name="key" size={22} />
      <div>
        <strong>Connect Gemini to start</strong>
        <p>Wisp uses your own Gemini API key. It takes about a minute.</p>
        <button
          className="btn btn-primary btn-small"
          onClick={() => void host().openSettings("welcome")}
        >
          Set up Wisp
        </button>
      </div>
    </div>
  );
}

function EmptyState() {
  const shortcuts = useSettings((s) => s.snapshot?.settings.globalShortcuts);
  return (
    <div className="empty">
      <p className="empty-title">Ask Wisp anything</p>
      <p className="empty-sub">Questions, rewrites, quick explanations.</p>
      {shortcuts && (
        <ul className="hints">
          {shortcuts.toggleIsland && (
            <li>
              <kbd>{shortcuts.toggleIsland}</kbd> show or hide Wisp
            </li>
          )}
          {shortcuts.selectToAsk && (
            <li>
              <kbd>{shortcuts.selectToAsk}</kbd> select an area to ask about
            </li>
          )}
          {shortcuts.dictation && (
            <li>
              <kbd>{shortcuts.dictation}</kbd> dictate text
            </li>
          )}
        </ul>
      )}
    </div>
  );
}

function Panel({ layout, onCollapse }: { layout: IslandLayout; onCollapse(): void }) {
  const state = useOrbState();
  const snapshot = useSettings((s) => s.snapshot);
  const {
    messages,
    requestActive,
    error,
    draft,
    setDraft,
    ask,
    cancel,
    retry,
    dismissError,
    clear,
  } = useConversation();
  const prompt = useRef<PromptBoxHandle>(null);
  const online = useOnline();

  useEffect(() => {
    prompt.current?.focus();
    return host().onFocusPrompt(() => prompt.current?.focus());
  }, []);

  const hasKey = snapshot?.hasApiKey ?? false;
  const pending = useCapture((c) => c.pending);
  const dictating = useDictation((d) => d.phase !== "idle");
  const setPending = useCapture((c) => c.setPending);

  const submit = () => {
    if (pending) {
      const prompt = draft.trim() || DEFAULT_SCREEN_PROMPT;
      setPending(null);
      void ask(prompt, {
        contextKind: "screen",
        attachments: [{ mimeType: pending.mimeType, dataBase64: pending.dataBase64 }],
        imagePreview: pending.dataUrl,
      });
    } else {
      void ask(draft);
    }
  };

  return (
    <section
      className={`panel ${layout.alignBottom ? "panel-bottom" : ""} ${layout.alignRight ? "panel-right" : ""}`}
      aria-label="Wisp"
    >
      <header className="panel-header" data-tauri-drag-region>
        <button
          className="orb-mini"
          onClick={onCollapse}
          aria-label="Collapse Wisp"
          title="Collapse (Esc)"
        >
          <Orb state={state} size={30} decorative />
        </button>
        <div className="panel-title" data-tauri-drag-region>
          <span className="panel-name" data-tauri-drag-region>
            Wisp
          </span>
          <span className={`status status-${state}`} aria-live="polite" data-tauri-drag-region>
            {ORB_LABELS[state]}
          </span>
        </div>
        <div className="panel-tools">
          {messages.length > 0 && (
            <button
              className="icon-btn"
              onClick={clear}
              aria-label="New conversation"
              title="New conversation"
            >
              <Icon name="plus" />
            </button>
          )}
          <button
            className="icon-btn"
            onClick={() => void useDictation.getState().start()}
            aria-label="Dictate"
            title={`Dictate${snapshot?.settings.globalShortcuts.dictation ? ` (${snapshot.settings.globalShortcuts.dictation})` : ""}`}
          >
            <Icon name="mic" />
          </button>
          <button
            className="icon-btn"
            onClick={() => void host().startCapture()}
            aria-label="Select an area to ask about"
            title={`Select an area to ask about${snapshot?.settings.globalShortcuts.selectToAsk ? ` (${snapshot.settings.globalShortcuts.selectToAsk})` : ""}`}
          >
            <Icon name="screen" />
          </button>
          <button
            className="icon-btn"
            onClick={() => void host().openSettings()}
            aria-label="Settings"
            title="Settings"
          >
            <Icon name="settings" />
          </button>
          <button
            className="icon-btn"
            onClick={onCollapse}
            aria-label="Collapse"
            title="Collapse (Esc)"
          >
            <Icon name="minimize" />
          </button>
          <button
            className="icon-btn"
            onClick={() => void host().hideIsland()}
            aria-label="Hide Wisp"
            title={`Hide Wisp${snapshot?.settings.globalShortcuts.toggleIsland ? ` (${snapshot.settings.globalShortcuts.toggleIsland})` : ""}`}
          >
            <Icon name="eye-off" />
          </button>
        </div>
      </header>

      {dictating ? (
        <DictationCard />
      ) : (
        <>
          <div className="panel-body">
            {!hasKey && snapshot && <SetupCard />}
            {messages.length === 0 && hasKey ? <EmptyState /> : <MessageList messages={messages} />}
          </div>

          {error && (
            <ErrorBanner
              error={error}
              onRetry={() => void retry()}
              onSettings={() => void host().openSettings(settingsTabFor(error.code))}
              onDismiss={dismissError}
            />
          )}

          <footer className="panel-footer">
            {pending && (
              <div className="attachment">
                <img src={pending.dataUrl} alt="Selected screen area" />
                <div className="attachment-text">
                  <strong>Screen area attached</strong>
                  Sent to Google Gemini only when you press Send.
                </div>
                <button
                  className="icon-btn"
                  onClick={() => setPending(null)}
                  aria-label="Remove screen area"
                  title="Remove"
                >
                  <Icon name="close" size={14} />
                </button>
              </div>
            )}
            <PromptBox
              ref={prompt}
              value={draft}
              onChange={setDraft}
              allowEmpty={!!pending}
              placeholder={pending ? "Ask about this area (optional)" : undefined}
              onSubmit={submit}
              onCancel={cancel}
              onEscape={onCollapse}
              busy={requestActive}
            />
            <p className="footnote">
              {online ? (
                <>
                  {snapshot?.settings.selectedModel ?? "Gemini"} · prompts are sent to Google Gemini
                </>
              ) : (
                <span className="offline">
                  <Icon name="warning" size={12} /> Offline. Local features still work.
                </span>
              )}
            </p>
          </footer>
        </>
      )}
    </section>
  );
}

export function Island() {
  const [layout, setLayout] = useState<IslandLayout>({
    expanded: false,
    alignRight: false,
    alignBottom: false,
  });

  const expand = useCallback(async () => {
    setLayout(await host().setIslandExpanded(true));
  }, []);
  const collapse = useCallback(async () => {
    setLayout(await host().setIslandExpanded(false));
  }, []);

  useEffect(() => {
    const offLayout = host().onIslandLayout(setLayout);
    const offFocus = host().onFocusPrompt(() => setLayout((l) => ({ ...l, expanded: true })));
    const offCapture = host().onCaptureReady((c) => {
      useCapture.getState().setPending(c);
      void host().setIslandExpanded(true).then(setLayout);
    });
    const offCaptureError = host().onCaptureError((e) => {
      useConversation.getState().setError(e);
      void host().setIslandExpanded(true).then(setLayout);
    });
    const offShortcut = host().onShortcut((action) => {
      if (action === "selectToAsk") {
        void host().startCapture();
        return;
      }
      if (action === "dictation") {
        void host().setIslandExpanded(true).then(setLayout);
        void useDictation.getState().toggle();
        return;
      }
      if (!shortcutHandlers.some((h) => h(action))) {
        toast("That feature arrives in a later build", "neutral", 2200);
      }
    });
    return () => {
      offLayout();
      offFocus();
      offShortcut();
      offCapture();
      offCaptureError();
    };
  }, []);

  return (
    <div className={`island ${layout.expanded ? "island-expanded" : ""}`}>
      {layout.expanded ? (
        <Panel layout={layout} onCollapse={() => void collapse()} />
      ) : (
        <CollapsedOrb onActivate={() => void expand()} />
      )}
      <ToastHost />
    </div>
  );
}
