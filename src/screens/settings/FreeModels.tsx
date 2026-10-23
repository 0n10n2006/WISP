import { useEffect, useState } from "react";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { presentError, toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { FreeModel, ModelUsage } from "../../services/types";
import { useSettings } from "../../state/settings";
import { ExternalLink, StatusLine, Toggle, useStatus } from "./common";

const OPENROUTER_KEYS = "https://openrouter.ai/keys";

function restingText(u: ModelUsage): string {
  if (!u.restingUntil) return "available";
  const at = new Date(u.restingUntil * 1000);
  const time = at.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  const tomorrow = at.toDateString() !== new Date().toDateString();
  return `used up until ${tomorrow ? "tomorrow " : ""}${time}`;
}

/** Automatic switching between free models, and today's use of each. */
export function RoutingSection() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  const [usage, setUsage] = useState<ModelUsage[]>([]);

  useEffect(() => {
    let live = true;
    const load = () =>
      host()
        .modelUsage()
        .then((u) => live && setUsage(u))
        .catch(() => {});
    void load();
    const t = setInterval(load, 10_000);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, []);

  if (!s) return null;
  return (
    <section className="group">
      <h3>Free models</h3>
      <p className="setting-desc">
        Google's free tier allows only a few requests a day on its best models. Wisp starts with the
        model above, then moves to Gemini Flash-Lite and Google's open Gemma models on the same key
        when one runs out (limits reset at midnight Pacific time). Web search and voice need Gemini.
      </p>
      <Toggle
        id="auto-fallback"
        label="Switch models automatically when one runs out"
        description="Off: Wisp only uses the model above and tells you when it's used up."
        checked={s.autoFallback}
        onChange={(autoFallback) => void update({ autoFallback })}
      />
      {usage.length > 0 ? (
        <ul className="usage-list" aria-label="Model use today">
          {usage.map((u) => (
            <li key={`${u.provider}:${u.model}`} className="usage-row">
              <span className="usage-name">{u.label}</span>
              <span className="muted">
                {u.requestsToday} today · {restingText(u)}
              </span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="setting-desc">No questions asked today yet.</p>
      )}
    </section>
  );
}

/** The optional OpenRouter key for simple jobs and as a last resort. */
export function OpenRouterSection() {
  const snapshot = useSettings((st) => st.snapshot);
  const update = useSettings((st) => st.update);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [models, setModels] = useState<FreeModel[]>([]);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const status = useStatus();
  const hasKey = snapshot?.hasOpenrouterKey ?? false;

  useEffect(() => {
    if (!hasKey) return;
    host()
      .testOpenrouterKey()
      .then(setModels)
      .catch(() => setModels([]));
  }, [hasKey]);

  if (!snapshot) return null;
  const s = snapshot.settings;

  const save = async () => {
    setBusy(true);
    status.clear();
    try {
      const free = await host().testOpenrouterKey(key.trim());
      await host().setOpenrouterKey(key.trim());
      setKey("");
      setModels(free);
      status.ok(`Key saved. ${free.length} free models available.`);
    } catch (e) {
      const err = toWispError(e);
      status.fail(`${presentError(err).title}. ${err.message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="group">
      <h3>OpenRouter (optional)</h3>
      <p className="setting-desc">
        Add a free OpenRouter key and Wisp uses its free models for simple jobs (clipboard help,
        rewrites, quick questions), saving your Gemini quota, and as a last resort when Gemini is
        used up. OpenRouter only ever gets text: never screenshots or audio. Free models may log
        prompts. <ExternalLink href={OPENROUTER_KEYS}>Get a key</ExternalLink>
      </p>
      <form
        className="key-form"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <label htmlFor="openrouter-key">
          {hasKey ? "Replace OpenRouter key" : "OpenRouter key"}
        </label>
        <div className="input-row">
          <input
            id="openrouter-key"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={key}
            placeholder="sk-or-…"
            onChange={(e) => setKey(e.target.value)}
          />
          <button className="btn btn-primary" disabled={busy || key.trim().length < 10}>
            Test & save
          </button>
        </div>
      </form>
      <StatusLine status={status.status} />
      {hasKey && (
        <>
          <p className="setting-desc">
            A key is saved in Windows Credential Manager. It is never shown.
          </p>
          <label htmlFor="openrouter-model">Free model</label>
          <select
            id="openrouter-model"
            value={s.openrouterModel}
            onChange={(e) => void update({ openrouterModel: e.target.value })}
          >
            <option value="">Automatic (largest free model)</option>
            {s.openrouterModel && !models.some((m) => m.id === s.openrouterModel) && (
              <option value={s.openrouterModel}>{s.openrouterModel}</option>
            )}
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name || m.id}
              </option>
            ))}
          </select>
          <Toggle
            id="simple-via-openrouter"
            label="Use OpenRouter for simple jobs"
            description="Off: OpenRouter is only used when the Gemini models are used up."
            checked={s.simpleViaOpenrouter}
            onChange={(simpleViaOpenrouter) => void update({ simpleViaOpenrouter })}
          />
          <div className="row-start">
            <button className="btn btn-danger-ghost" onClick={() => setConfirmRemove(true)}>
              Remove OpenRouter key
            </button>
          </div>
        </>
      )}
      {confirmRemove && (
        <ConfirmDialog
          title="Remove OpenRouter key?"
          confirmLabel="Remove key"
          destructive
          onCancel={() => setConfirmRemove(false)}
          onConfirm={async () => {
            setConfirmRemove(false);
            try {
              await host().removeOpenrouterKey();
              setModels([]);
              status.ok("OpenRouter key removed from this PC.");
            } catch (e) {
              status.fail(toWispError(e));
            }
          }}
        >
          <p>Wisp will use only your Gemini key. The key itself stays valid on OpenRouter.</p>
        </ConfirmDialog>
      )}
    </section>
  );
}
