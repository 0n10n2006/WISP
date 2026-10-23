import { useEffect, useState } from "react";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { ShortcutInput } from "../../components/ShortcutInput";
import { presentError, toWispError } from "../../services/errors";
import { host } from "../../services/host";
import {
  chordLabel,
  HOLD_CHOICES,
  type HoldChord,
  type ModelInfo,
  type Settings,
  type ShortcutAction,
  type Shortcuts,
  type UpdateInfo,
} from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, ExternalLink, LINKS, StatusLine, Toggle, useStatus } from "./common";
import { OpenRouterSection, RoutingSection } from "./FreeModels";

function useSettingsValue(): Settings | null {
  return useSettings((s) => s.snapshot?.settings ?? null);
}

export function GeneralTab() {
  const s = useSettingsValue();
  const update = useSettings((st) => st.update);
  const { status, fail, clear } = useStatus();
  if (!s) return null;
  const set = async (patch: Partial<Settings>) => {
    clear();
    const err = await update(patch);
    if (err) fail(err);
  };
  return (
    <div className="tab">
      <h2>General</h2>
      <section className="group">
        <Toggle
          id="show-notch"
          label="Show the notch"
          description="Turn off to hide Wisp's glow at the top of the screen. The shortcuts and the tray icon still open Wisp."
          checked={!s.tucked}
          onChange={(shown) => void set({ tucked: !shown })}
        />
      </section>
      <section className="group">
        <h3>Appearance</h3>
        <Choice
          name="theme"
          label="Theme"
          value={s.theme}
          options={[
            { value: "system", label: "System" },
            { value: "dark", label: "Dark" },
            { value: "light", label: "Light" },
          ]}
          onChange={(theme) => void set({ theme })}
        />
        <Choice
          name="motion"
          label="Motion"
          value={s.motion}
          options={[
            { value: "system", label: "System" },
            { value: "reduced", label: "Reduced" },
            { value: "full", label: "Full" },
          ]}
          onChange={(motion) => void set({ motion })}
        />
        <Choice
          name="scale"
          label="Size of Wisp"
          value={String(s.islandScale)}
          options={[
            { value: "1", label: "Default" },
            { value: "1.15", label: "Larger" },
            { value: "1.3", label: "Largest" },
          ]}
          onChange={(v) => void set({ islandScale: Number(v) })}
        />
      </section>
      <section className="group">
        <h3>Window</h3>
        <Toggle
          id="launch-at-login"
          label="Start Wisp when you sign in"
          checked={s.launchAtLogin}
          onChange={(launchAtLogin) => void set({ launchAtLogin })}
        />
        <Toggle
          id="start-hidden"
          label="Start hidden"
          description="When starting at sign-in, stay in the tray until you press the shortcut."
          checked={s.startHidden}
          onChange={(startHidden) => void set({ startHidden })}
        />
      </section>
      <StatusLine status={status} />
    </div>
  );
}

export function ModelTab() {
  const snapshot = useSettings((s) => s.snapshot);
  const update = useSettings((s) => s.update);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const keyStatus = useStatus();
  const testStatus = useStatus();
  const hasKey = snapshot?.hasApiKey ?? false;

  useEffect(() => {
    if (!hasKey) return;
    host()
      .listModels()
      .then(setModels)
      .catch(() => setModels([]));
  }, [hasKey]);

  if (!snapshot) return null;
  const model = snapshot.settings.selectedModel;

  const saveKey = async () => {
    setBusy(true);
    keyStatus.clear();
    try {
      const report = await host().testConnection(key.trim());
      await host().setApiKey(key.trim());
      setKey("");
      setModels(report.models);
      keyStatus.ok(
        report.modelAvailable
          ? "Key saved and working."
          : `Key saved. "${model}" isn't available for this key — pick another model below.`,
      );
    } catch (e) {
      const err = toWispError(e);
      keyStatus.fail(`${presentError(err).title}. ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  const test = async () => {
    setBusy(true);
    testStatus.clear();
    try {
      const r = await host().testConnection();
      setModels(r.models);
      if (r.modelAvailable) testStatus.ok(`Connected in ${r.latencyMs} ms. ${model} is available.`);
      else
        testStatus.fail(
          `Connected, but ${model} isn't available for this key. Choose another model.`,
        );
    } catch (e) {
      const err = toWispError(e);
      testStatus.fail(`${presentError(err).title}. ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tab">
      <h2>Model</h2>
      <section className="group">
        <h3>Gemini API key</h3>
        <p className="setting-desc">
          {hasKey
            ? "A key is saved in Windows Credential Manager. For your security it is never shown."
            : "No key saved yet. Wisp needs your own Gemini API key to answer questions."}{" "}
          <ExternalLink href={LINKS.getKey}>Get a key</ExternalLink>
        </p>
        <form
          className="key-form"
          onSubmit={(e) => {
            e.preventDefault();
            void saveKey();
          }}
        >
          <label htmlFor="replace-key">{hasKey ? "Replace key" : "API key"}</label>
          <div className="input-row">
            <input
              id="replace-key"
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={key}
              placeholder="AIza…"
              onChange={(e) => setKey(e.target.value)}
            />
            <button className="btn btn-primary" disabled={busy || key.trim().length < 10}>
              Test & save
            </button>
          </div>
        </form>
        <StatusLine status={keyStatus.status} />
        {hasKey && (
          <div className="row-start">
            <button className="btn" onClick={() => void test()} disabled={busy}>
              Test connection
            </button>
            <button className="btn btn-danger-ghost" onClick={() => setConfirmRemove(true)}>
              Remove key
            </button>
          </div>
        )}
        <StatusLine status={testStatus.status} />
      </section>

      <section className="group">
        <h3>Model</h3>
        <label htmlFor="model-select">Model used for answers</label>
        <select
          id="model-select"
          value={model}
          onChange={(e) => void update({ selectedModel: e.target.value })}
          disabled={!hasKey}
        >
          {!models.some((m) => m.id === model) && <option value={model}>{model}</option>}
          {models.map((m) => (
            <option key={m.id} value={m.id}>
              {m.displayName} ({m.id})
            </option>
          ))}
        </select>
        <p className="setting-desc">
          Only models your key can use are listed. Free-tier quotas differ per model and change over
          time. <ExternalLink href={LINKS.rateLimits}>Rate limits</ExternalLink>{" "}
          <ExternalLink href={LINKS.billing}>Billing</ExternalLink>
        </p>
      </section>

      <RoutingSection />
      <OpenRouterSection />

      {confirmRemove && (
        <ConfirmDialog
          title="Remove API key?"
          confirmLabel="Remove key"
          destructive
          onCancel={() => setConfirmRemove(false)}
          onConfirm={async () => {
            setConfirmRemove(false);
            try {
              await host().removeApiKey();
              setModels([]);
              keyStatus.ok("Key removed from this PC.");
            } catch (e) {
              keyStatus.fail(toWispError(e));
            }
          }}
        >
          <p>
            Wisp won't be able to answer questions until you add a key again. The key itself stays
            valid in Google AI Studio.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}

const SHORTCUT_LABELS: Record<ShortcutAction, { label: string; desc: string }> = {
  toggleIsland: { label: "Open the command bar", desc: "Opens Wisp with the bar ready to type." },
  selectToAsk: {
    label: "Circle to ask",
    desc: "Circle or drag around anything on screen and ask about it.",
  },
  dictation: { label: "Dictation", desc: "Start or stop dictating." },
  clipboardAssist: { label: "Clipboard", desc: "Clipboard history, or help with copied text." },
  voice: { label: "Ask by voice", desc: "Speak a question." },
  tuck: { label: "Tuck Wisp away", desc: "Hide the notch completely, or bring it back." },
};

export function ShortcutsTab() {
  const snapshot = useSettings((s) => s.snapshot);
  const update = useSettings((s) => s.update);
  const apply = useSettings((s) => s.apply);
  const [draft, setDraft] = useState<Shortcuts | null>(null);
  const { status, ok, fail, clear } = useStatus();
  if (!snapshot) return null;
  const current = draft ?? snapshot.settings.globalShortcuts;
  const dirty =
    draft !== null && JSON.stringify(draft) !== JSON.stringify(snapshot.settings.globalShortcuts);

  return (
    <div className="tab">
      <h2>Shortcuts</h2>
      <p className="setting-desc">
        Global shortcuts work from any app. Use Ctrl, Alt or Win with a key. Click a shortcut and
        press the new combination (Wisp's own shortcuts pause while you do); Backspace disables it.
      </p>
      {snapshot.shortcutIssues.length > 0 && (
        <div className="banner banner-warning" role="alert">
          <div className="banner-body">
            <strong>Some shortcuts couldn't be registered</strong>
            {snapshot.shortcutIssues.map((i) => (
              <p key={i.action}>{i.message} You can still use the notch or tray menu.</p>
            ))}
          </div>
        </div>
      )}
      <section className="group">
        {(Object.keys(SHORTCUT_LABELS) as ShortcutAction[]).map((action) => (
          <div className="setting-row" key={action}>
            <div className="setting-text">
              <label htmlFor={`sc-${action}`}>{SHORTCUT_LABELS[action].label}</label>
              <p className="setting-desc">{SHORTCUT_LABELS[action].desc}</p>
            </div>
            <ShortcutInput
              id={`sc-${action}`}
              label={SHORTCUT_LABELS[action].label}
              value={current[action]}
              onChange={(v) => {
                clear();
                setDraft({ ...current, [action]: v });
              }}
            />
          </div>
        ))}
      </section>
      <div className="row-start">
        <button
          className="btn btn-primary"
          disabled={!dirty}
          onClick={async () => {
            const err = await update({ globalShortcuts: current });
            if (err) fail(err);
            else {
              setDraft(null);
              ok("Shortcuts saved.");
            }
          }}
        >
          Save shortcuts
        </button>
        {dirty && (
          <button className="btn btn-ghost" onClick={() => setDraft(null)}>
            Discard changes
          </button>
        )}
        <button
          className="btn"
          onClick={async () => {
            try {
              apply(await host().resetShortcuts());
              setDraft(null);
              ok("Shortcuts reset to defaults.");
            } catch (e) {
              fail(toWispError(e));
            }
          }}
        >
          Reset to defaults
        </button>
      </div>
      <StatusLine status={status} />
      <HoldKeys />
    </div>
  );
}

/** Which two modifier keys, held on their own, dictate or ask by voice. */
function HoldKeys() {
  const s = useSettingsValue();
  const update = useSettings((st) => st.update);
  const { status, fail, clear } = useStatus();
  if (!s) return null;
  const pick = async (patch: Partial<Settings>) => {
    clear();
    const err = await update(patch);
    if (err) fail(err);
  };
  const select = (
    id: string,
    value: HoldChord,
    other: HoldChord,
    key: "holdDictate" | "holdAsk",
  ) => (
    <select
      id={id}
      className="select-inline"
      value={value}
      disabled={!s.holdToTalk || !s.voiceEnabled}
      onChange={(e) => void pick({ [key]: e.target.value as HoldChord })}
    >
      {HOLD_CHOICES.map((c) => (
        <option key={c} value={c} disabled={c !== "off" && c === other}>
          {chordLabel(c)}
        </option>
      ))}
    </select>
  );
  return (
    <section className="group">
      <h3>Hold to talk</h3>
      <p className="setting-desc">
        Hold two modifier keys on their own to talk; let go to finish. Pressing any other key while
        holding them keeps your normal shortcuts working.
        {!s.holdToTalk && " Hold-to-talk is off in Voice & dictation."}
      </p>
      <div className="setting-row">
        <div className="setting-text">
          <label htmlFor="hold-dictate">Hold to dictate into any app</label>
        </div>
        {select("hold-dictate", s.holdDictate, s.holdAsk, "holdDictate")}
      </div>
      <div className="setting-row">
        <div className="setting-text">
          <label htmlFor="hold-ask">Hold to ask by voice</label>
        </div>
        {select("hold-ask", s.holdAsk, s.holdDictate, "holdAsk")}
      </div>
      <StatusLine status={status} />
    </section>
  );
}

export function PrivacyTab() {
  const s = useSettingsValue();
  const update = useSettings((st) => st.update);
  if (!s) return null;
  return (
    <div className="tab">
      <h2>Privacy</h2>
      <section className="group">
        <h3>What leaves your PC</h3>
        <ul className="plain-list">
          <li>
            <strong>Sent to Google Gemini when you ask:</strong> your prompt, recent messages in the
            current conversation, and — only if you explicitly include them — a selected screen
            region, clipboard text, or dictated audio. Enabled memory notes are included unless you
            turn memory off. With app awareness on (below), the names of the app in front, your open
            apps and installed apps are included too; window titles only if you allow them.
          </li>
          <li>
            <strong>Only when you use them:</strong> web answers and research briefs use Google
            Search through Gemini; “Show me” and screen replicas send one screenshot of the current
            screen to Gemini; “Google Lens” on a circled area sends just that area to Google Lens
            and opens the results in your browser; weather sends the place you picked to Open-Meteo;
            the speed test downloads from speed.cloudflare.com; update checks contact GitHub.
          </li>
          <li>
            <strong>Stays on this PC:</strong> settings, reminders, memory, clipboard history (off
            by default), launcher and agent history, and request history (metadata only unless you
            opt in below). Your API key is stored in Windows Credential Manager.
          </li>
          <li>
            <strong>Never collected:</strong> Wisp has no analytics or telemetry. Nothing is sent to
            the Wisp developers.
          </li>
          <li>
            Deleting data in Wisp removes it from this PC only. It doesn't erase anything Google has
            already received; see Google's terms for how long it is kept.
          </li>
        </ul>
        <p className="links">
          <ExternalLink href={LINKS.terms}>Gemini API terms</ExternalLink>
          <ExternalLink href={LINKS.billing}>Free vs paid data use</ExternalLink>
          <ExternalLink href={LINKS.privacy}>Google privacy policy</ExternalLink>
        </p>
      </section>
      <section className="group">
        <h3>Controls</h3>
        <Toggle
          id="use-memory"
          label="Use memory to personalise answers"
          description="Include your saved memory notes in requests."
          checked={s.useMemory}
          onChange={(useMemory) => void update({ useMemory })}
        />
        <Toggle
          id="clear-on-exit"
          label="Clear history when Wisp closes"
          description="Forgets request history, the action log and unpinned clipboard history every time you quit Wisp or shut down the PC. Reminders, memory and settings are kept."
          checked={s.clearHistoryOnExit}
          onChange={(clearHistoryOnExit) => void update({ clearHistoryOnExit })}
        />
        <Toggle
          id="content-history"
          label="Keep conversation text in history"
          description="Off by default. When off, history keeps only time, type and status."
          checked={s.contentHistory}
          onChange={(contentHistory) => void update({ contentHistory })}
        />
        <div className="setting-row">
          <div className="setting-text">
            <label htmlFor="retention">Keep history for</label>
            <p className="setting-desc" id="retention-desc">
              Older entries are deleted automatically when Wisp starts.
            </p>
          </div>
          <select
            id="retention"
            className="select-inline"
            aria-describedby="retention-desc"
            value={s.historyRetentionDays}
            onChange={(e) => void update({ historyRetentionDays: Number(e.target.value) })}
          >
            <option value={0}>Don't keep history</option>
            <option value={7}>7 days</option>
            <option value={30}>30 days</option>
            <option value={90}>90 days</option>
            <option value={365}>1 year</option>
          </select>
        </div>
        <Toggle
          id="screen-context"
          label="Allow screen features"
          description="Circle to ask, Show me and screen replicas. The screen is captured only when you ask, and selections are sent only after you confirm."
          checked={s.screenContextEnabled}
          onChange={(screenContextEnabled) => void update({ screenContextEnabled })}
        />
        <Toggle
          id="clipboard-history"
          label="Clipboard history"
          description="Off by default. Keeps your last 100 copied texts and links on this PC; skips anything password managers mark as private."
          checked={s.clipboardHistory}
          onChange={(clipboardHistory) => void update({ clipboardHistory })}
        />
        <Toggle
          id="app-awareness"
          label="Let Wisp know which apps are open"
          description="Sends the names of the app in front, your open apps and your installed apps (not their contents) with each question, so Wisp can answer about them, open them and switch to them."
          checked={s.appAwareness}
          onChange={(appAwareness) => void update({ appAwareness })}
        />
        <Toggle
          id="share-titles"
          label="Include window titles"
          description="Off by default. Titles can contain document names, email subjects or page titles."
          checked={s.shareWindowTitles}
          disabled={!s.appAwareness}
          onChange={(shareWindowTitles) => void update({ shareWindowTitles })}
        />
        <Toggle
          id="retain-audio"
          label="Keep dictation audio"
          description="Off by default. Audio is discarded right after transcription."
          checked={s.retainAudio}
          onChange={(retainAudio) => void update({ retainAudio })}
        />
      </section>
    </div>
  );
}

export function AboutTab() {
  const snapshot = useSettings((s) => s.snapshot);
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const { status, ok, fail, clear } = useStatus();
  const check = async () => {
    setChecking(true);
    clear();
    try {
      const u = await host().checkForUpdates();
      setUpdate(u);
      if (u.available) ok(`Wisp ${u.latest} is available.`);
      else ok("You're on the latest version.");
    } catch (e) {
      fail(toWispError(e));
    } finally {
      setChecking(false);
    }
  };
  return (
    <div className="tab">
      <h2>About Wisp</h2>
      <p>Version {snapshot?.version}</p>
      <p className="setting-desc">
        Wisp is an independent, open project. It uses the Google Gemini API with your own key and
        runs entirely on your PC otherwise.
      </p>
      {snapshot?.recoveredFrom && (
        <div className="banner banner-warning" role="status">
          <div className="banner-body">
            <strong>Settings were reset</strong>
            <p>
              Your settings file couldn't be read, so defaults were restored. The old file was kept
              at {snapshot.recoveredFrom}.
            </p>
          </div>
        </div>
      )}
      <section className="group">
        <h3>Updates</h3>
        <p className="setting-desc">
          Wisp only checks for updates when you ask. It looks up the latest release on the project's
          GitHub page and never downloads or installs anything by itself.
        </p>
        <div className="row-start">
          <button className="btn" disabled={checking} onClick={() => void check()}>
            {checking ? "Checking…" : "Check for updates"}
          </button>
          {update?.available && update.url && (
            <button
              className="btn btn-primary"
              onClick={() => void host().openExternal(update.url!)}
            >
              Open release {update.latest}
            </button>
          )}
        </div>
        <StatusLine status={status} />
      </section>
      <div className="row-start">
        <button className="btn btn-danger-ghost" onClick={() => void host().quit()}>
          Quit Wisp
        </button>
      </div>
    </div>
  );
}
