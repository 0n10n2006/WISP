import { act, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import type { AudioRecorder } from "../../src/services/recorder";
import { useActivity } from "../../src/state/activity";
import { useVoice } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

function recorder(): AudioRecorder {
  return {
    start: vi.fn(async () => {}),
    stop: vi.fn(async () => ({ base64: "AAAA", mimeType: "audio/webm", durationMs: 1500 })),
    cancel: vi.fn(),
  };
}

describe("voice conversation", () => {
  beforeEach(() => {
    useVoice.getState().cancel();
    useActivity.setState({ listening: false });
  });

  it("listens, asks with voice context, and answers in text only", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /ask by voice/i }));
    expect(
      await screen.findByText("Listening…", { selector: ".dictation-title" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(ask.mock.calls[0]![0]).toMatchObject({
      contextKind: "voice",
      prompt: "This is a simulated transcript from the browser preview.",
    });
    expect(await screen.findByText(/simulated answer/i, {}, { timeout: 5000 })).toBeInTheDocument();
    // Wisp never talks back.
    expect("speechSynthesis" in window && vi.isMockFunction(window.speechSynthesis?.speak)).toBe(
      false,
    );
    expect(useActivity.getState().listening).toBe(false);
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

  it("uses Windows' words straight away when it's sure, without uploading audio", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      speech: { text: "What time is it in Tokyo", confidence: "high" },
    });
    const rec = recorder();
    vi.spyOn(h, "createRecorder").mockReturnValue(rec);
    const transcribe = vi.spyOn(h, "transcribe");
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /ask by voice/i }));
    // Words appear while speaking.
    expect(await screen.findByText("What time is it in Tokyo")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(ask.mock.calls[0]![0].prompt).toBe("What time is it in Tokyo");
    expect(transcribe).not.toHaveBeenCalled();
    expect(rec.cancel).toHaveBeenCalled();
  });

  it("falls back to Gemini when Windows isn't sure", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      speech: { text: "mumble", confidence: "low" },
    });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    const transcribe = vi.spyOn(h, "transcribe");
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /ask by voice/i }));
    await screen.findByText("Listening…", { selector: ".dictation-title" });
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(transcribe).toHaveBeenCalledOnce();
    expect(ask.mock.calls[0]![0].prompt).toBe(
      "This is a simulated transcript from the browser preview.",
    );
  });

  it("uses only Gemini when that's the chosen engine", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      speech: { text: "hello there", confidence: "high" },
    });
    await h.updateSettings({ speechEngine: "gemini" });
    const start = vi.spyOn(h, "speechStart");
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    const transcribe = vi.spyOn(h, "transcribe");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /ask by voice/i }));
    await screen.findByText("Listening…", { selector: ".dictation-title" });
    await user.click(screen.getByRole("button", { name: "Done" }));
    await waitFor(() => expect(transcribe).toHaveBeenCalledOnce(), { timeout: 3000 });
    expect(start).not.toHaveBeenCalled();
  });
});
