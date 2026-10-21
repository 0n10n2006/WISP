import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { ToastHost } from "../../components/Toast";
import { useOrbState } from "../../hooks/useOrbState";
import { prefetchCatalogue } from "../../lib/catalogue";
import { lazyWithPreload, whenIdle } from "../../lib/preload";
import { host } from "../../services/host";
import type { HoldEvent, ShortcutAction } from "../../services/types";
import { useClipboardAssist, useDueReminders } from "../../state/assist";
import { useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useDictation } from "../../state/dictation";
import { useNotch } from "../../state/notch";
import { ORB_LABELS } from "../../state/orb";
import { useSettings } from "../../state/settings";
import { useVoice } from "../../state/voice";
import { preloadDashboard } from "../dashboard/load";
import { useConfirm } from "./confirm";
import { closeBar as close, openBar as open } from "./control";
import { Peek } from "./Peek";

/** The open bar is a separate chunk, fetched while idle so opening is instant. */
const Bar = lazyWithPreload(() => import("./Bar"));

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

function Idle() {
  const mode = useNotch((n) => n.mode);
  const state = useOrbState();
  const leaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const listening = useDictation((d) => d.phase === "listening" || d.phase === "transcribing");
  const voice = useVoice((v) => v.phase === "listening" || v.phase === "transcribing");
  const sticky = listening || voice;
  const sliverStyle = useSettings((s) => s.snapshot?.settings.sliverStyle ?? "glow");

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
      data-style={sliverStyle}
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
    if (e.phase === "start") void dictation.start(true);
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
  if (e.phase === "start") void voice.start(true);
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

  // Fetch the bar and the dashboard while the PC is idle.
  useEffect(() => {
    whenIdle(() => {
      void Bar.preload();
      preloadDashboard(useNotch.getState().tab);
      prefetchCatalogue();
    });
  }, []);

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
        const closeOnBlur = useSettings.getState().snapshot?.settings.closeOnBlur ?? true;
        if (closeOnBlur && useNotch.getState().mode === "open" && !busy()) close();
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
      {mode === "open" ? (
        <Suspense fallback={<div className="bar-wrap" aria-busy="true" />}>
          <Bar />
        </Suspense>
      ) : (
        <Idle />
      )}
      <ToastHost />
    </div>
  );
}
