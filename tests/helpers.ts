import { act } from "@testing-library/react";
import { setHost, type Host } from "../src/services/host";
import { createMockHost, type MockHostOptions } from "../src/services/mockHost";
import { useClipboardAssist, useDueReminders } from "../src/state/assist";
import { useCapture } from "../src/state/capture";
import { useConversation } from "../src/state/conversation";
import { useDictation } from "../src/state/dictation";
import { useNotch } from "../src/state/notch";
import { useSettings } from "../src/state/settings";
import { useConfirm } from "../src/screens/notch/actions";

/** Fresh mock host + reset stores for each test. */
export async function setupMockHost(
  opts: MockHostOptions & { withKey?: boolean; onboarded?: boolean } = {},
): Promise<Host> {
  const h = createMockHost({ tokenDelay: 0, persist: false, ...opts });
  setHost(h);
  useConversation.setState({
    messages: [],
    requestActive: false,
    error: null,
    draft: "",
    successFlash: false,
    lastFailed: null,
  });
  useSettings.setState({ snapshot: null, loadError: null });
  useClipboardAssist.setState({ open: false, loading: false, text: null, error: null });
  useDueReminders.setState({ due: [] });
  useCapture.setState({ pending: null });
  useNotch.setState({
    mode: "sliver",
    tab: "today",
    query: "",
    web: false,
    think: false,
    timer: null,
  });
  useConfirm.setState({ pending: null });
  useDictation.getState().cancel();
  if (opts.withKey) await h.setApiKey("AIzaTEST-key-0000000000000000000");
  if (opts.onboarded) await h.updateSettings({ onboardingComplete: true });
  await act(async () => {
    await useSettings.getState().load();
  });
  return h;
}
