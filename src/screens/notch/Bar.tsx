import { useEffect, useRef, useState } from "react";
import { ClipboardCard } from "../../components/ClipboardCard";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Icon, type IconName } from "../../components/Icon";
import { MessageList } from "../../components/MessageList";
import { ReminderBanner } from "../../components/ReminderBanner";
import { useOnline } from "../../hooks/useOnline";
import { chipsFor } from "../../lib/chips";
import { host } from "../../services/host";
import { askAboutClipboard, useClipboardAssist } from "../../state/assist";
import { runCaptureAction, useCapture, type CaptureAction } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useNotch } from "../../state/notch";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { Dashboard } from "../dashboard/load";
import { useConfirm } from "./actions";
import { busy } from "./busy";
import { CommandBar, type CommandBarHandle } from "./CommandBar";
import { blurCloseAllowed, closeBar, lastHeight, rememberBarHeight } from "./control";
import { Sources } from "./Sources";

/** How long the bar waits after the mouse leaves before folding away. */
export const LEAVE_FOLD_MS = 900;
/** Text typed this recently counts as "still working on it". */
const TYPING_GRACE_MS = 3000;

/** "Ctrl+Alt+Space" as people read it on Windows. */
export function shortcutLabel(accel: string): string {
  return accel.replace(/CommandOrControl|CmdOrCtrl/g, "Ctrl").replace(/Super|Meta|Cmd/g, "Win");
}

/** Tallest the open bar may grow (CSS px); its body scrolls beyond this. */
const MAX_BAR_HEIGHT = 620;
/** Room below the bar for its shadow (must match `.notch-mode-open` padding). */
const SHADOW_ROOM = 26;

function settingsTabFor(code: string): string {
  if (code === "permission_denied") return "privacy";
  return "model";
}

function SetupCard() {
  return (
    <div className="setup-card">
      <Icon name="key" size={20} />
      <div>
        <strong>Connect Gemini to ask questions</strong>
        <p>Apps, controls and the dashboard work without it. Asking uses your own Gemini key.</p>
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

function ConfirmStrip() {
  const pending = useConfirm((c) => c.pending);
  const clear = useConfirm((c) => c.clear);
  const ok = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (pending) ok.current?.focus();
  }, [pending]);
  if (!pending) return null;
  return (
    <div className="confirm-strip" role="alertdialog" aria-label="Confirm">
      <Icon name="warning" size={16} />
      <span>{pending.text}</span>
      <button className="btn btn-small" onClick={clear}>
        Cancel
      </button>
      <button
        ref={ok}
        className="btn btn-small btn-danger"
        onClick={() => {
          const run = pending.run;
          clear();
          void run();
        }}
      >
        {pending.label}
      </button>
    </div>
  );
}

function ErrorRow() {
  const error = useConversation((c) => c.error);
  const retry = useConversation((c) => c.retry);
  const dismissError = useConversation((c) => c.dismissError);
  const model = useSettings((s) => s.snapshot?.settings.selectedModel);
  if (!error) return null;
  return (
    <ErrorBanner
      error={error}
      onRetry={() => {
        useNotch.getState().setQuery("");
        void retry();
      }}
      onSettings={() => void host().openSettings(settingsTabFor(error.code))}
      onDismiss={dismissError}
      currentModel={model}
      onSwitchModel={async (m) => {
        const err = await useSettings.getState().update({ selectedModel: m });
        if (!err) {
          toast(`Switched to ${m}`, "success");
          useNotch.getState().setQuery("");
          void retry();
        }
      }}
    />
  );
}

function Conversation() {
  const messages = useConversation((c) => c.messages);
  const clear = useConversation((c) => c.clear);
  const last = messages[messages.length - 1];
  return (
    <div className="bar-chat">
      <div className="bar-chat-head">
        <span className="muted">Conversation</span>
        <button className="btn btn-small btn-ghost" onClick={clear}>
          <Icon name="plus" size={14} /> New
        </button>
      </div>
      <MessageList messages={messages} />
      {last?.role === "model" && last.status === "done" && (last.via || last.note) ? (
        <p className="answer-meta">
          {last.via && <>Answered by {last.via}. </>}
          {last.note}
        </p>
      ) : null}
      {last?.role === "model" && last.sources?.length ? <Sources sources={last.sources} /> : null}
    </div>
  );
}

/** Clipboard help plus suggestions for the app the user came from. */
function Chips() {
  const [app, setApp] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    void host()
      .foregroundApp()
      .then((a) => alive && setApp(a))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);
  const chips = chipsFor(app);
  return (
    <div className="chips" role="toolbar" aria-label="Quick help">
      <button
        className="chip"
        onClick={() => void useClipboardAssist.getState().openAssist()}
        title="Summarise, rewrite or explain what you copied"
      >
        <Icon name="clipboard" size={14} /> Clipboard help
      </button>
      {chips.map((c) => (
        <button
          key={c.id}
          className="chip"
          onClick={() => void askAboutClipboard(c.prompt, c.label)}
          title="Uses the text you copied"
        >
          {c.label}
        </button>
      ))}
    </div>
  );
}

const CAPTURE_ACTIONS: { id: CaptureAction; label: string; icon: IconName; title: string }[] = [
  { id: "answer", label: "Answer it", icon: "spark", title: "Answer the question shown (Gemini)" },
  { id: "explain", label: "Explain", icon: "info", title: "Explain what's shown (Gemini)" },
  {
    id: "search",
    label: "Search the web",
    icon: "globe",
    title: "Gemini identifies it and searches the web, with sources",
  },
  { id: "text", label: "Copy text", icon: "copy", title: "Read the text and copy it (Gemini)" },
  {
    id: "lens",
    label: "Google Lens",
    icon: "search",
    title: "Reverse image search: opens Google Lens results in your browser",
  },
];

function PendingCapture({ sendCapture }: { sendCapture(): void }) {
  const pending = useCapture((c) => c.pending);
  const web = useNotch((n) => n.web);
  const think = useNotch((n) => n.think);
  const setPending = useCapture((c) => c.setPending);
  if (!pending) return null;
  return (
    <div className="attachment">
      <img src={pending.dataUrl} alt="Selected screen area" />
      <div className="attachment-text">
        <strong>Screen area attached</strong>
        Type a question, or pick an action. Nothing is sent until you do.
        <div className="capture-actions" role="group" aria-label="Do something with this area">
          {CAPTURE_ACTIONS.map((a) => (
            <button
              key={a.id}
              className="chip"
              title={a.title}
              onClick={() => void runCaptureAction(a.id, web, think)}
            >
              <Icon name={a.icon} size={13} /> {a.label}
            </button>
          ))}
        </div>
      </div>
      <button className="btn btn-small btn-primary" onClick={sendCapture}>
        Send
      </button>
      <button
        className="icon-btn"
        onClick={() => setPending(null)}
        aria-label="Remove screen area"
        title="Remove"
      >
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}

/** Esc: dismiss what's in front (a confirmation, a running answer), else fold. */
function escape() {
  const conversation = useConversation.getState();
  if (useConfirm.getState().pending) useConfirm.getState().clear();
  else if (conversation.requestActive) conversation.cancel();
  else closeBar();
}

function FoldHint() {
  const accel = useSettings((s) => s.snapshot?.settings.globalShortcuts.toggleIsland);
  return <p className="bar-hint">Esc{accel ? ` or ${shortcutLabel(accel)}` : ""} to fold away</p>;
}

export default function Bar() {
  const root = useRef<HTMLElement>(null);
  const input = useRef<CommandBarHandle>(null);
  const query = useNotch((n) => n.query);
  const messages = useConversation((c) => c.messages);
  const clipboardOpen = useClipboardAssist((c) => c.open);
  const snapshot = useSettings((s) => s.snapshot);
  const online = useOnline();
  const [dismissed, setDismissed] = useState<string[]>([]);

  useEffect(() => {
    input.current?.focus();
    return host().onFocusPrompt(() => input.current?.focus());
  }, []);

  // Fold away once the mouse has left, unless the user is still mid-task.
  const typedAt = useRef(0);
  useEffect(() => {
    typedAt.current = Date.now();
  }, [query]);
  const leaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelLeave = () => {
    if (leaveTimer.current) clearTimeout(leaveTimer.current);
    leaveTimer.current = null;
  };
  const onLeave = () => {
    cancelLeave();
    if (!(useSettings.getState().snapshot?.settings.foldOnLeave ?? true)) return;
    leaveTimer.current = setTimeout(() => {
      leaveTimer.current = null;
      const typing =
        !!useNotch.getState().query.trim() && Date.now() - typedAt.current < TYPING_GRACE_MS;
      const picking = document.activeElement?.tagName === "SELECT";
      if (blurCloseAllowed() && !busy() && !typing && !picking) closeBar();
    }, LEAVE_FOLD_MS);
  };
  useEffect(() => cancelLeave, []);

  // Esc folds the bar wherever focus is (after clicking a dashboard tab, or
  // with nothing focused at all); a running answer is stopped first.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      if (root.current?.contains(e.target as Node)) return; // the bar's own handler
      e.preventDefault();
      escape();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  // Size the window to the content so the bar never covers more than it
  // shows. Heights are reported only once they have been stable for two
  // frames, snapped to 4 px, so opening causes at most one resize.
  useEffect(() => {
    const el = root.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    // The window already opened at the remembered height.
    let last = lastHeight() ?? 0;
    let frame = 0;
    let candidate = 0;
    let stableFrames = 0;
    const report = (h: number) => {
      if (h === last) return;
      last = h;
      rememberBarHeight(h);
      void host().setIslandMode("open", h);
    };
    const measure = () => {
      const raw = Math.min(Math.ceil(el.getBoundingClientRect().height), MAX_BAR_HEIGHT);
      const h = Math.ceil((raw + SHADOW_ROOM) / 4) * 4;
      if (h !== candidate) {
        candidate = h;
        stableFrames = 0;
      } else stableFrames += 1;
      if (stableFrames >= 1) {
        frame = 0;
        report(h);
      } else frame = requestAnimationFrame(measure);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    const ro = new ResizeObserver(schedule);
    ro.observe(el);
    schedule();
    return () => {
      ro.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  const hasKey = snapshot?.hasApiKey ?? false;
  const warnings = (snapshot?.warnings ?? []).filter((w) => !dismissed.includes(w));
  const showChat = messages.length > 0;

  let body;
  if (clipboardOpen) body = <ClipboardCard />;
  else if (query.trim())
    body = null; // results render inside the command bar
  else if (showChat)
    body = (
      <>
        <Chips />
        <Conversation />
      </>
    );
  else
    body = (
      <>
        {hasKey && <Chips />}
        <Dashboard />
      </>
    );

  return (
    <div className="bar-wrap" onPointerLeave={onLeave} onPointerEnter={cancelLeave}>
      <section
        ref={root}
        className="bar"
        data-testid="bar"
        aria-label="Wisp"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !e.defaultPrevented) {
            e.preventDefault();
            escape();
          }
        }}
      >
        <CommandBar ref={input} onClose={closeBar} />
        <ConfirmStrip />
        <ErrorRow />
        <ReminderBanner />
        {warnings.map((w) => (
          <div key={w} className="banner banner-warning" role="status">
            <div className="banner-body">
              <p>{w}</p>
            </div>
            <button
              className="icon-btn banner-close"
              aria-label="Dismiss notice"
              onClick={() => setDismissed((d) => [...d, w])}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        ))}
        <PendingCapture sendCapture={() => input.current?.submit()} />
        {body !== null && (
          <div className="bar-body">
            {!hasKey && snapshot && !query && <SetupCard />}
            {body}
          </div>
        )}
        {!online && (
          <p className="bar-offline">
            <Icon name="warning" size={12} /> Offline. Apps, controls and reminders still work.
          </p>
        )}
        <FoldHint />
      </section>
    </div>
  );
}
