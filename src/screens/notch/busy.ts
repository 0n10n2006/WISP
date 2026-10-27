// Whether something in the open bar still needs the user, so it mustn't fold
// away on its own (on blur or when the mouse leaves).

import { useCapture } from "../../state/capture";
import { useConversation } from "../../state/conversation";
import { useVoice } from "../../state/voice";
import { useConfirm } from "./confirm";

export function busy(): boolean {
  return (
    useConversation.getState().requestActive ||
    useVoice.getState().listening ||
    !!useConfirm.getState().pending ||
    !!useCapture.getState().pending ||
    useConversation.getState().messages.some((m) => m.actions?.some((a) => a.confirmId))
  );
}
