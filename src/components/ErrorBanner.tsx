import { presentError, retryHint, type WispError } from "../services/errors";
import { Icon } from "./Icon";

interface Props {
  error: WispError;
  onRetry?: () => void;
  onSettings?: () => void;
  onDismiss?: () => void;
}

/** Persistent inline message for errors that need action (not a toast). */
export function ErrorBanner({ error, onRetry, onSettings, onDismiss }: Props) {
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
          {error.code === "quota_exhausted" && onRetry && (
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
