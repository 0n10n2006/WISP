// Error model shared with the Rust host, plus user-facing microcopy.

export type ErrorCode =
  | "missing_key"
  | "invalid_key"
  | "rate_limited"
  | "quota_exhausted"
  | "model_unavailable"
  | "bad_request"
  | "blocked"
  | "offline"
  | "timeout"
  | "server_error"
  | "malformed_response"
  | "cancelled"
  | "invalid"
  | "permission_denied"
  | "storage"
  | "unsupported"
  | "internal";

export interface WispError {
  code: ErrorCode;
  message: string;
  retryAfterSecs?: number;
  retryable: boolean;
}

const CODES = new Set<string>([
  "missing_key",
  "invalid_key",
  "rate_limited",
  "quota_exhausted",
  "model_unavailable",
  "bad_request",
  "blocked",
  "offline",
  "timeout",
  "server_error",
  "malformed_response",
  "cancelled",
  "invalid",
  "permission_denied",
  "storage",
  "unsupported",
  "internal",
]);

export function isWispError(e: unknown): e is WispError {
  return (
    typeof e === "object" &&
    e !== null &&
    typeof (e as WispError).code === "string" &&
    CODES.has((e as WispError).code) &&
    typeof (e as WispError).message === "string"
  );
}

/** Normalise anything thrown across IPC into a WispError. */
export function toWispError(e: unknown): WispError {
  if (isWispError(e)) return e;
  const message =
    typeof e === "string" ? e : e instanceof Error ? e.message : "Something went wrong.";
  return { code: "internal", message, retryable: false };
}

export type ErrorSeverity = "warning" | "error";

export interface ErrorPresentation {
  title: string;
  /** The single most useful next step. */
  action: "retry" | "settings" | "wait" | "none";
  actionLabel?: string;
  severity: ErrorSeverity;
}

export function presentError(e: WispError): ErrorPresentation {
  switch (e.code) {
    case "missing_key":
      return {
        title: "Add your Gemini API key",
        action: "settings",
        actionLabel: "Open settings",
        severity: "warning",
      };
    case "invalid_key":
      return {
        title: "API key rejected",
        action: "settings",
        actionLabel: "Check key",
        severity: "error",
      };
    case "rate_limited":
      return {
        title: "Slow down a moment",
        action: "retry",
        actionLabel: "Try again",
        severity: "warning",
      };
    case "quota_exhausted":
      return {
        title: "Quota used up",
        action: "settings",
        actionLabel: "Switch model",
        severity: "warning",
      };
    case "model_unavailable":
      return {
        title: "Model unavailable",
        action: "settings",
        actionLabel: "Choose model",
        severity: "error",
      };
    case "offline":
      return {
        title: "You're offline",
        action: "retry",
        actionLabel: "Try again",
        severity: "warning",
      };
    case "timeout":
    case "server_error":
    case "malformed_response":
      return {
        title: "Gemini didn't answer",
        action: "retry",
        actionLabel: "Try again",
        severity: "error",
      };
    case "blocked":
      return { title: "Gemini declined", action: "none", severity: "warning" };
    case "permission_denied":
      return {
        title: "Permission needed",
        action: "settings",
        actionLabel: "Open settings",
        severity: "warning",
      };
    case "cancelled":
      return { title: "Stopped", action: "retry", actionLabel: "Try again", severity: "warning" };
    case "invalid":
    case "bad_request":
      return { title: "Couldn't do that", action: "none", severity: "error" };
    case "unsupported":
      return { title: "Not available", action: "none", severity: "warning" };
    default:
      return {
        title: "Something went wrong",
        action: "retry",
        actionLabel: "Try again",
        severity: "error",
      };
  }
}

/** "Try again in 30 s" style hint when the provider told us. */
export function retryHint(e: WispError): string | null {
  if (!e.retryAfterSecs) return null;
  const s = e.retryAfterSecs;
  if (s < 90) return `Try again in about ${s} s.`;
  const m = Math.ceil(s / 60);
  if (m < 90) return `Try again in about ${m} min.`;
  return `Try again in about ${Math.ceil(m / 60)} h.`;
}
