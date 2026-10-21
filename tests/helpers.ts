import { act } from "@testing-library/react";
import { setHost, type Host } from "../src/services/host";
import { createMockHost, type MockHostOptions } from "../src/services/mockHost";
import { useConversation } from "../src/state/conversation";
import { useSettings } from "../src/state/settings";

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
  if (opts.withKey) await h.setApiKey("AIzaTEST-key-0000000000000000000");
  if (opts.onboarded) await h.updateSettings({ onboardingComplete: true });
  await act(async () => {
    await useSettings.getState().load();
  });
  return h;
}
