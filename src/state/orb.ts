// Status model for the notch. A status indicator first, mascot second:
// each state has a distinct glyph and label, never colour alone.

export type OrbState = "idle" | "listening" | "thinking" | "success" | "warning" | "error";

export interface OrbInputs {
  listening: boolean;
  requestActive: boolean;
  errorSeverity: "warning" | "error" | null;
  successFlash: boolean;
}

/** Highest-priority active state wins: capture states are never hidden. */
export function deriveOrbState(i: OrbInputs): OrbState {
  if (i.listening) return "listening";
  if (i.requestActive) return "thinking";
  if (i.errorSeverity === "error") return "error";
  if (i.errorSeverity === "warning") return "warning";
  if (i.successFlash) return "success";
  return "idle";
}

export const ORB_LABELS: Record<OrbState, string> = {
  idle: "Ready",
  listening: "Listening…",
  thinking: "Thinking…",
  success: "Done",
  warning: "Needs attention",
  error: "Something went wrong",
};

export const SUCCESS_FLASH_MS = 1200;
