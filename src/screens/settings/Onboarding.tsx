import { useState } from "react";
import { Icon } from "../../components/Icon";
import { WispMark } from "../../components/WispMark";
import { presentError, toWispError, type WispError } from "../../services/errors";
import { host } from "../../services/host";
import type { ModelInfo } from "../../services/types";
import { useSettings } from "../../state/settings";
import { ExternalLink, LINKS } from "./common";

type Step = "intro" | "key" | "model" | "done";

export function Onboarding({ onFinish }: { onFinish(): void }) {
  const snapshot = useSettings((s) => s.snapshot);
  const update = useSettings((s) => s.update);
  const [step, setStep] = useState<Step>(snapshot?.hasApiKey ? "model" : "intro");
  const [key, setKey] = useState("");
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<WispError | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [model, setModel] = useState(snapshot?.settings.selectedModel ?? "gemini-2.5-flash");

  const saveKey = async () => {
    setTesting(true);
    setError(null);
    try {
      const report = await host().testConnection(key.trim(), model);
      await host().setApiKey(key.trim());
      setKey("");
      setModels(report.models);
      if (!report.modelAvailable && report.models[0]) {
        const flash = report.models.find((m) => m.id.includes("flash")) ?? report.models[0];
        setModel(flash.id);
      }
      setStep("model");
    } catch (e) {
      setError(toWispError(e));
    } finally {
      setTesting(false);
    }
  };

  const loadModels = async () => {
    if (models.length) return;
    try {
      setModels(await host().listModels());
    } catch (e) {
      setError(toWispError(e));
    }
  };

  const finish = async () => {
    const err = await update({ selectedModel: model, onboardingComplete: true });
    if (err) setError(err);
    else setStep("done");
  };

  return (
    <div className="onboarding">
      <div className="onboarding-hero">
        <WispMark state={step === "done" ? "success" : "idle"} size={64} />
        <div>
          <h1>Meet Wisp</h1>
          <p className="lead">A small assistant that stays out of the way until you need it.</p>
        </div>
      </div>

      <ol className="steps" aria-label="Setup steps">
        {(["intro", "key", "model", "done"] as Step[]).map((s, i) => (
          <li
            key={s}
            aria-current={s === step ? "step" : undefined}
            className={s === step ? "current" : ""}
          >
            {i + 1}. {{ intro: "About", key: "API key", model: "Model", done: "Ready" }[s]}
          </li>
        ))}
      </ol>

      {step === "intro" && (
        <section className="card">
          <h2>How Wisp works</h2>
          <ul className="plain-list">
            <li>
              Wisp uses <strong>Google Gemini</strong> with <strong>your own API key</strong>.
              Nothing is shared through a Wisp server.
            </li>
            <li>
              When you ask something, your prompt (and a screen region or clipboard text, only if
              you choose to include it) is sent to Google. Settings, reminders and memory stay on
              this PC.
            </li>
            <li>
              The Gemini free tier has daily and per-minute limits that Google can change. Wisp will
              tell you when you hit them — it never promises unlimited use.
            </li>
            <li>
              On the free tier, Google may use prompts and responses to improve its products. Avoid
              sending sensitive information, or use a paid key.
            </li>
          </ul>
          <p className="links">
            <ExternalLink href={LINKS.terms}>Gemini API terms & data use</ExternalLink>
            <ExternalLink href={LINKS.rateLimits}>Current rate limits</ExternalLink>
          </p>
          <div className="row-end">
            <button className="btn btn-primary" onClick={() => setStep("key")}>
              Continue <Icon name="chevron-right" size={16} />
            </button>
          </div>
        </section>
      )}

      {step === "key" && (
        <section className="card">
          <h2>Add your Gemini API key</h2>
          <p>
            Create a free key in Google AI Studio, then paste it here. Wisp stores it in Windows
            Credential Manager and never shows it again.
          </p>
          <p>
            <ExternalLink href={LINKS.getKey}>Get a key in Google AI Studio</ExternalLink>
          </p>
          <form
            className="key-form"
            onSubmit={(e) => {
              e.preventDefault();
              void saveKey();
            }}
          >
            <label htmlFor="api-key">API key</label>
            <div className="input-row">
              <input
                id="api-key"
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={key}
                onChange={(e) => setKey(e.target.value)}
                placeholder="AIza…"
                aria-describedby={error ? "key-error" : undefined}
              />
              <button className="btn btn-primary" disabled={testing || key.trim().length < 10}>
                {testing ? "Testing…" : "Test & save"}
              </button>
            </div>
            {error && (
              <p id="key-error" className="field-error" role="alert">
                <strong>{presentError(error).title}.</strong> {error.message}
              </p>
            )}
          </form>
          <div className="row-between">
            <button className="btn btn-ghost" onClick={() => setStep("intro")}>
              Back
            </button>
          </div>
        </section>
      )}

      {step === "model" && (
        <section className="card">
          <h2>Choose a model</h2>
          <p>
            Flash models are fast and the best fit for the free tier. You can change this any time.
          </p>
          <label htmlFor="model">Model</label>
          <select
            id="model"
            value={model}
            onFocus={() => void loadModels()}
            onChange={(e) => setModel(e.target.value)}
          >
            {!models.some((m) => m.id === model) && <option value={model}>{model}</option>}
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.displayName} ({m.id})
              </option>
            ))}
          </select>
          {error && (
            <p className="field-error" role="alert">
              {error.message}
            </p>
          )}
          <div className="row-end">
            <button className="btn btn-primary" onClick={() => void finish()}>
              Finish setup
            </button>
          </div>
        </section>
      )}

      {step === "done" && (
        <section className="card">
          <h2>You're set</h2>
          <p>
            Wisp rests as a thin glow at the top of your screen. Hover it for a peek, click it to
            open the command bar, or use these:
          </p>
          <ul className="hints">
            <li>
              <kbd>{snapshot?.settings.globalShortcuts.toggleIsland}</kbd> open the command bar:
              ask, open apps, run commands
            </li>
            <li>
              <kbd>{snapshot?.settings.globalShortcuts.selectToAsk}</kbd> circle anything on screen
              to ask about it
            </li>
            <li>
              Hold <kbd>Ctrl</kbd>+<kbd>Win</kbd> to dictate into any app · hold <kbd>Win</kbd>+
              <kbd>Alt</kbd> to ask by voice
            </li>
            <li>
              <kbd>{snapshot?.settings.globalShortcuts.tuck}</kbd> tuck Wisp away completely
            </li>
          </ul>
          <p className="muted">You can change shortcuts in Settings → Shortcuts.</p>
          <div className="row-end">
            <button className="btn btn-primary" onClick={onFinish}>
              Done
            </button>
          </div>
        </section>
      )}
    </div>
  );
}
