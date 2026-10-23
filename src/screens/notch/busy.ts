// Whether something in the open bar still needs the user, so it mustn't fold
// away on its own (on blur or when the mouse leaves).

import { useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useDictation } from "../../state/dictation";
import { useVoice } from "../../state/voice";
import { useConfirm } from "./confirm";

export function busy(): boolean {
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
