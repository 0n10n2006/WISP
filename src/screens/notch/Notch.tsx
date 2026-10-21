import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { ClipboardCard } from "../../components/ClipboardCard";
import { DictationCard } from "../../components/DictationCard";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Icon } from "../../components/Icon";
import { MessageList } from "../../components/MessageList";
import { ReminderBanner } from "../../components/ReminderBanner";
import { ToastHost } from "../../components/Toast";
import { VoiceCard } from "../../components/VoiceCard";
import { useOnline } from "../../hooks/useOnline";
import { useOrbState } from "../../hooks/useOrbState";
import { host } from "../../services/host";
import type { HoldEvent, ShortcutAction } from "../../services/types";
import { askAboutClipboard, useClipboardAssist, useDueReminders } from "../../state/assist";
import { chipsFor } from "../../lib/chips";
import { useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useDictation } from "../../state/dictation";
import { useNotch } from "../../state/notch";
import { ORB_LABELS } from "../../state/orb";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { useVoice } from "../../state/voice";
import { useConfirm } from "./actions";
import { CommandBar, type CommandBarHandle } from "./CommandBar";
import { Peek } from "./Peek";
import { Sources } from "./Sources";

const Dashboard = lazy(() => import("../dashboard/Dashboard"));

/** Tallest the open bar may grow (CSS px); its body scrolls beyond this. */
const MAX_BAR_HEIGHT = 620;

function settingsTabFor(code: string): string {
  if (code === "permission_denied") return "privacy";
  return "model";
}

function open() {
  useNotch.getState().setMode("open");
  void host().setIslandMode("open");
  host().prewarm();
}

function close() {
  useConfirm.getState().clear();
  void host().hideIsland();
}

/** Clicking elsewhere folds the bar away unless something needs the user. */
function busy(): boolean {
  const d = useDictation.getState().phase;
  const v = useVoice.getState().phase;
  return (
    useConversation.getState().requestActive ||
    (d !== "idle" && d !== "error") ||
    v === "listening" ||
    v === "transcribing" ||
    !!useConfirm.getState().pending ||
    !!useCapture.getState().pending ||
    useConversation.getState().messages.some((m) => m.actions?.some((a) => a.confirmId))
  );
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

function PendingCapture({ sendCapture }: { sendCapture(): void }) {
  const pending = useCapture((c) => c.pending);
  const setPending = useCapture((c) => c.setPending);
  if (!pending) return null;
  return (
    <div className="attachment">
      <img src={pending.dataUrl} alt="Selected screen area" />
      <div className="attachment-text">
        <strong>Screen area attached</strong>
        Type a question if you like. Sent to Google Gemini only when you press Send.
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

function Bar() {
  const root = useRef<HTMLElement>(null);
  const input = useRef<CommandBarHandle>(null);
  const query = useNotch((n) => n.query);
  const messages = useConversation((c) => c.messages);
  const dictating = useDictation((d) => d.phase !== "idle");
  const voiceActive = useVoice((v) => v.phase !== "idle");
  const clipboardOpen = useClipboardAssist((c) => c.open);
  const snapshot = useSettings((s) => s.snapshot);
  const online = useOnline();
  const [dismissed, setDismissed] = useState<string[]>([]);

  useEffect(() => {
    input.current?.focus();
    return host().onFocusPrompt(() => input.current?.focus());
  }, []);

  // Size the window to the content so the bar never covers more than it shows.
  useEffect(() => {
    const el = root.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    let last = 0;
    let frame = 0;
    const measure = () => {
      frame = 0;
      const h = Math.min(Math.ceil(el.getBoundingClientRect().height), MAX_BAR_HEIGHT) + 18;
      if (Math.abs(h - last) > 1) {
        last = h;
        void host().setIslandMode("open", h);
      }
    };
    const ro = new ResizeObserver(() => {
      if (!frame) frame = requestAnimationFrame(measure);
    });
    ro.observe(el);
    measure();
    return () => {
      ro.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  const hasKey = snapshot?.hasApiKey ?? false;
  const warnings = (snapshot?.warnings ?? []).filter((w) => !dismissed.includes(w));
  const showChat = messages.length > 0;

  let body;
  if (dictating) body = <DictationCard />;
  else if (voiceActive) body = <VoiceCard />;
  else if (clipboardOpen) body = <ClipboardCard />;
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
        <Suspense fallback={<div className="dash-loading" aria-busy="true" />}>
          <Dashboard />
        </Suspense>
      </>
    );

  return (
    <section
      ref={root}
      className="bar"
      aria-label="Wisp"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.defaultPrevented) {
          e.preventDefault();
          if (useConfirm.getState().pending) useConfirm.getState().clear();
          else close();
        }
      }}
    >
      <CommandBar ref={input} onClose={close} />
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
          {!hasKey && snapshot && !query && !dictating && !voiceActive && <SetupCard />}
          {body}
        </div>
      )}
      {!online && (
        <p className="bar-offline">
          <Icon name="warning" size={12} /> Offline. Apps, controls and reminders still work.
        </p>
      )}
    </section>
  );
}

function Idle() {
  const mode = useNotch((n) => n.mode);
  const state = useOrbState();
  const leaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const listening = useDictation((d) => d.phase === "listening" || d.phase === "transcribing");
  const voice = useVoice((v) => v.phase === "listening" || v.phase === "transcribing");
  const sticky = listening || voice;

  const peek = useCallback(() => {
    if (leaveTimer.current) clearTimeout(leaveTimer.current);
    if (useNotch.getState().mode === "sliver") {
      useNotch.getState().setMode("peek");
      void host().setIslandMode("peek");
    }
  }, []);
  const unpeek = useCallback(() => {
    if (leaveTimer.current) clearTimeout(leaveTimer.current);
    leaveTimer.current = setTimeout(() => {
      if (useNotch.getState().mode === "peek" && !sticky) {
        useNotch.getState().setMode("sliver");
        void host().setIslandMode("sliver");
      }
    }, 450);
  }, [sticky]);
  useEffect(() => () => void (leaveTimer.current && clearTimeout(leaveTimer.current)), []);

  return (
    <button
      className={`notch-idle notch-${mode}`}
      data-state={state}
      onPointerEnter={peek}
      onPointerLeave={unpeek}
      onFocus={peek}
      onClick={open}
      aria-label={`Open Wisp — ${ORB_LABELS[state]}`}
      title="Open Wisp"
    >
      {mode === "peek" ? <Peek /> : <SliverGlow />}
    </button>
  );
}

function SliverGlow() {
  const timer = useNotch((n) => n.timer);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!timer) return;
    const t = setInterval(() => setNow(Date.now()), 5000);
    return () => clearInterval(t);
  }, [timer]);
  const progress = timer
    ? Math.min(1, Math.max(0, (now - timer.startedAt) / (timer.endsAt - timer.startedAt)))
    : null;
  return (
    <span className="sliver" aria-hidden>
      {progress !== null && (
        <span className="sliver-progress" style={{ width: `${progress * 100}%` }} />
      )}
    </span>
  );
}

/** Hold-to-talk: Ctrl+Win dictates into the app in front, Win+Alt asks by voice. */
function onHold(e: HoldEvent) {
  const dictation = useDictation.getState();
  const voice = useVoice.getState();
  if (e.chord === "dictate") {
    if (e.phase === "start") void dictation.start();
    if (e.phase === "cancel") {
      dictation.cancel();
      void host().hideIsland();
    }
    if (e.phase === "stop") {
      void dictation.stop(true).then(() => {
        const after = useDictation.getState();
        if (after.phase === "idle") void host().hideIsland();
        else open();
      });
    }
    return;
  }
  if (e.phase === "start") void voice.start();
  if (e.phase === "cancel") {
    voice.cancel();
    void host().hideIsland();
  }
  if (e.phase === "stop") {
    open();
    void voice.finish();
  }
}

export function Notch() {
  const mode = useNotch((n) => n.mode);
  const accent = useSettings((s) => s.snapshot?.settings.accent ?? "cyan");

  useEffect(() => {
    document.documentElement.dataset.accent = accent;
  }, [accent]);

  useEffect(() => {
    const h = host();
    const setMode = useNotch.getState().setMode;
    const offs = [
      h.onIslandLayout((l) => setMode(l.mode)),
      h.onFocusPrompt(() => {
        setMode("open");
        h.prewarm();
      }),
      h.onBlur(() => {
        if (useNotch.getState().mode === "open" && !busy()) close();
      }),
      h.onReminderDue((t) => {
        useDueReminders.getState().push(t);
        if (t.title === "Focus session finished") useNotch.getState().finishTimer();
      }),
      h.onCaptureReady((c) => {
        useCapture.getState().setPending(c);
        open();
      }),
      h.onCaptureError((e) => {
        useConversation.getState().setError(e);
        open();
      }),
      h.onHold(onHold),
      h.onShortcut((action: ShortcutAction) => {
        if (action === "selectToAsk") return void h.startCapture();
        open();
        if (action === "clipboardAssist") {
          if (useSettings.getState().snapshot?.settings.clipboardHistory) {
            useNotch.getState().setTab("clipboard");
            useConversation.getState().clear();
          } else void useClipboardAssist.getState().openAssist();
        }
        if (action === "voice") void useVoice.getState().toggle();
        if (action === "dictation") void useDictation.getState().toggle();
      }),
    ];
    return () => offs.forEach((off) => off());
  }, []);

  // A failed or cancelled question comes back to the bar for editing.
  useEffect(
    () =>
      useConversation.subscribe((c) => {
        if (!c.draft) return;
        const draft = c.draft;
        useConversation.getState().setDraft("");
        if (!useNotch.getState().query) useNotch.getState().setQuery(draft);
      }),
    [],
  );

  // A finished timer clears itself even if no reminder event arrives.
  const timer = useNotch((n) => n.timer);
  useEffect(() => {
    if (!timer) return;
    const left = timer.endsAt - Date.now();
    const t = setTimeout(() => useNotch.getState().finishTimer(), Math.max(0, left));
    return () => clearTimeout(t);
  }, [timer]);

  return (
    <div className={`notch notch-mode-${mode}`}>
      {mode === "open" ? <Bar /> : <Idle />}
      <ToastHost />
    </div>
  );
}
