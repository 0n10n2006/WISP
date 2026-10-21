import { useEffect, useRef, useState } from "react";
import { ClipboardCard } from "../../components/ClipboardCard";
import { DictationCard } from "../../components/DictationCard";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Icon } from "../../components/Icon";
import { MessageList } from "../../components/MessageList";
import { ReminderBanner } from "../../components/ReminderBanner";
import { VoiceCard } from "../../components/VoiceCard";
import { useOnline } from "../../hooks/useOnline";
import { chipsFor } from "../../lib/chips";
import { host } from "../../services/host";
import { askAboutClipboard, useClipboardAssist } from "../../state/assist";
import { useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useDictation } from "../../state/dictation";
import { useNotch } from "../../state/notch";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { useVoice } from "../../state/voice";
import { Dashboard } from "../dashboard/load";
import { useConfirm } from "./actions";
import { CommandBar, type CommandBarHandle } from "./CommandBar";
import { closeBar, lastHeight, rememberBarHeight } from "./control";
import { Sources } from "./Sources";

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

export default function Bar() {
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
        <Dashboard />
      </>
    );

  return (
    <div className="bar-wrap">
      <section
        ref={root}
        className="bar"
        data-testid="bar"
        aria-label="Wisp"
        onKeyDown={(e) => {
          if (e.key === "Escape" && !e.defaultPrevented) {
            e.preventDefault();
            if (useConfirm.getState().pending) useConfirm.getState().clear();
            else closeBar();
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
    </div>
  );
}
