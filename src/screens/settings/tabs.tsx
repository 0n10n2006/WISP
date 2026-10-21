import { useEffect, useState } from "react";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { ShortcutInput } from "../../components/ShortcutInput";
import { presentError, toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { ModelInfo, Settings, ShortcutAction, Shortcuts } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, ExternalLink, LINKS, StatusLine, Toggle, useStatus } from "./common";

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
      </section>
      <section className="group">
        <h3>Window</h3>
        <Toggle
          id="always-on-top"
          label="Keep Wisp above other windows"
          checked={s.alwaysOnTop}
          onChange={(alwaysOnTop) => void set({ alwaysOnTop })}
        />
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
  toggleIsland: { label: "Show or hide Wisp", desc: "Opens Wisp with the prompt ready." },
  selectToAsk: { label: "Select to ask", desc: "Select an area of the screen and ask about it." },
  dictation: { label: "Dictation", desc: "Start or stop dictating text." },
  clipboardAssist: { label: "Clipboard help", desc: "Summarize, rewrite or explain copied text." },
  voice: { label: "Voice conversation", desc: "Push to talk with Wisp." },
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
        Global shortcuts work from any app. Use Ctrl, Alt or Win with a key. Select a shortcut and
        press the new combination; Backspace disables it.
      </p>
      {snapshot.shortcutIssues.length > 0 && (
        <div className="banner banner-warning" role="alert">
          <div className="banner-body">
            <strong>Some shortcuts couldn't be registered</strong>
            {snapshot.shortcutIssues.map((i) => (
              <p key={i.action}>{i.message} You can still use the orb or tray menu.</p>
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
    </div>
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
            turn memory off.
          </li>
          <li>
            <strong>Stays on this PC:</strong> settings, reminders, memory, and request history
            (metadata only unless you opt in below). Your API key is stored in Windows Credential
            Manager.
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
          label="Allow Select to ask"
          description="Screen regions are captured only when you press the shortcut and sent only after you confirm."
          checked={s.screenContextEnabled}
          onChange={(screenContextEnabled) => void update({ screenContextEnabled })}
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
      <div className="row-start">
        <button className="btn btn-danger-ghost" onClick={() => void host().quit()}>
          Quit Wisp
        </button>
      </div>
    </div>
  );
}
