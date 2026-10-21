import { useEffect, useState } from "react";
import { presentError, retryHint, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { ModelInfo } from "../services/types";
import { Icon } from "./Icon";

interface Props {
  error: WispError;
  onRetry?: () => void;
  onSettings?: () => void;
  onDismiss?: () => void;
  /** Current model; enables "switch model" for quota/availability errors. */
  currentModel?: string;
  onSwitchModel?: (modelId: string) => void;
}

/** Pick the best alternative: prefer Flash models, then anything else. */
export function pickAlternative(models: ModelInfo[], current: string): ModelInfo | null {
  const others = models.filter((m) => m.id !== current && !m.id.includes("preview"));
  return (
    others.find((m) => m.id.includes("flash") && !m.id.includes("lite")) ??
    others.find((m) => m.id.includes("flash")) ??
    others[0] ??
    null
  );
}

function useAlternative(error: WispError, current?: string): ModelInfo | null {
  const [alt, setAlt] = useState<ModelInfo | null>(null);
  const wanted =
    !!current && (error.code === "model_unavailable" || error.code === "quota_exhausted");
  useEffect(() => {
    if (!wanted || !current) return;
    let live = true;
    host()
      .listModels()
      .then((models) => live && setAlt(pickAlternative(models, current)))
      .catch(() => live && setAlt(null));
    return () => {
      live = false;
    };
  }, [wanted, current]);
  return wanted ? alt : null;
}

/** Persistent inline message for errors that need action (not a toast). */
export function ErrorBanner({
  error,
  onRetry,
  onSettings,
  onDismiss,
  currentModel,
  onSwitchModel,
}: Props) {
  const alternative = useAlternative(error, onSwitchModel ? currentModel : undefined);
  const p = presentError(error);
  const hint = retryHint(error);
  const showRetry = p.action === "retry" && onRetry;
  const showSettings = (p.action === "settings" || error.code === "quota_exhausted") && onSettings;
  return (
    <div className={`banner banner-${p.severity}`} role="alert">
      <span className="banner-icon">
        <Icon name={p.severity === "error" ? "error" : "warning"} size={18} />
      </span>
      <div className="banner-body">
        <strong>{p.title}</strong>
        <p>
          {error.message}
          {hint && <> {hint}</>}
        </p>
        <div className="banner-actions">
          {alternative && onSwitchModel && (
            <button
              className="btn btn-small btn-primary"
              onClick={() => onSwitchModel(alternative.id)}
            >
              Switch to {alternative.displayName} and retry
            </button>
          )}
          {showRetry && (
            <button className="btn btn-small" onClick={onRetry}>
              <Icon name="retry" size={14} /> {p.actionLabel}
            </button>
          )}
          {showSettings && (
            <button className="btn btn-small" onClick={onSettings}>
              {p.actionLabel ?? "Open settings"}
            </button>
          )}
          {error.code === "quota_exhausted" && onRetry && !alternative && (
            <button className="btn btn-small btn-ghost" onClick={onRetry}>
              Try again
            </button>
          )}
        </div>
      </div>
      {onDismiss && (
        <button className="icon-btn banner-close" onClick={onDismiss} aria-label="Dismiss message">
          <Icon name="close" size={14} />
        </button>
      )}
    </div>
  );
}
