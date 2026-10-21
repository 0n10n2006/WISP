import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import type { AudioRecorder } from "../../src/services/recorder";
import { useActivity } from "../../src/state/activity";
import { useVoice } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

class FakeUtterance {
  text: string;
  voice: unknown = null;
  rate = 1;
  onend: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(text: string) {
    this.text = text;
  }
}

const spoken: FakeUtterance[] = [];
const synth = {
  speak: vi.fn((u: FakeUtterance) => spoken.push(u)),
  cancel: vi.fn(),
  getVoices: () => [],
  addEventListener: () => {},
  removeEventListener: () => {},
};

function recorder(): AudioRecorder {
  return {
    start: vi.fn(async () => {}),
    stop: vi.fn(async () => ({ base64: "AAAA", mimeType: "audio/webm", durationMs: 1500 })),
    cancel: vi.fn(),
  };
}

describe("voice conversation", () => {
  beforeEach(() => {
    spoken.length = 0;
    vi.stubGlobal("speechSynthesis", synth);
    vi.stubGlobal("SpeechSynthesisUtterance", FakeUtterance);
    useVoice.getState().cancel();
    useActivity.setState({ listening: false, speaking: false });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("listens, asks with voice context, and reads the answer aloud", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /talk to wisp/i }));
    expect(
      await screen.findByText("Listening…", { selector: ".dictation-title" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(ask.mock.calls[0]![0]).toMatchObject({
      contextKind: "voice",
      prompt: "This is a simulated transcript from the browser preview.",
    });
    await waitFor(() => expect(spoken).toHaveLength(1), { timeout: 5000 });
    expect(useActivity.getState().speaking).toBe(true);
    expect(await screen.findByText("Speaking…", { selector: ".speaking-bar" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop" }));
    expect(synth.cancel).toHaveBeenCalled();
    expect(useActivity.getState().speaking).toBe(false);
  });

  it("respects the voice setting", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ voiceEnabled: false });
    await act(async () => {
      const { useSettings } = await import("../../src/state/settings");
      await useSettings.getState().load();
    });
    await act(() => useVoice.getState().start());
    expect(useVoice.getState().phase).toBe("error");
    expect(useVoice.getState().error?.message).toMatch(/turned off/i);
  });
});
