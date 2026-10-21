import { presentError } from "../services/errors";
import { useActivity } from "../state/activity";
import { useConversation } from "../state/conversation";
import { deriveOrbState, type OrbState } from "../state/orb";
import { useSettings } from "../state/settings";

export function useOrbState(): OrbState {
  const listening = useActivity((s) => s.listening);
  const requestActive = useConversation((s) => s.requestActive);
  const error = useConversation((s) => s.error);
  const successFlash = useConversation((s) => s.successFlash);
  const hasKey = useSettings((s) => s.snapshot?.hasApiKey ?? true);
  return deriveOrbState({
    listening,
    requestActive,
    errorSeverity: error ? presentError(error).severity : hasKey ? null : "warning",
    successFlash,
  });
}
