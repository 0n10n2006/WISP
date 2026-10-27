import { act, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useNotch } from "../../src/state/notch";
import { useSettings } from "../../src/state/settings";
import { spokenQuery, useVoice, VOICE_SEND_MS } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

describe("voice (Windows voice typing)", () => {
  it("the dictate button starts voice typing in the app you came from", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const typing = vi.spyOn(h, "voiceTyping");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Dictate" }));
    expect(typing).toHaveBeenCalledWith("app");
  });

  it("asks by voice: the words appear in the bar and are asked once they stop", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "What time is it in Tokyo.",
    });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    expect(useVoice.getState().listening).toBe(true);
    const box = screen.getByRole("combobox", { name: /ask wisp/i });
    await waitFor(() => expect(box).toHaveValue("What time is it in Tokyo."));
    expect(screen.getByText(/asking when you stop/i)).toBeInTheDocument();
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: VOICE_SEND_MS + 2000 });
    // The full stop Windows adds is dropped.
    expect(ask.mock.calls[0]![0].prompt).toBe("What time is it in Tokyo");
    expect(useVoice.getState().listening).toBe(false);
  });

  it("typing cancels the automatic send so you can edit", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "Summarise my week",
    });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    const box = screen.getByRole("combobox", { name: /ask wisp/i });
    await waitFor(() => expect(box).toHaveValue("Summarise my week"));
    await user.type(box, " please");
    expect(useVoice.getState().listening).toBe(false);
    await new Promise((r) => setTimeout(r, VOICE_SEND_MS + 300));
    expect(ask).not.toHaveBeenCalled();
    await user.type(box, "{Enter}");
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
    expect(ask.mock.calls[0]![0].prompt).toBe("Summarise my week please");
  });

  it("a spoken command runs like a typed one", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "Timer 25.",
    });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    await waitFor(() => expect(useNotch.getState().timer).not.toBeNull(), {
      timeout: VOICE_SEND_MS + 2000,
    });
    expect(ask).not.toHaveBeenCalled();
  });

  it("says when voice is turned off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await act(async () => void (await useSettings.getState().update({ voiceEnabled: false })));
    const typing = vi.spyOn(h, "voiceTyping");
    render(<App />);
    await act(() => useVoice.getState().dictate());
    expect(typing).not.toHaveBeenCalled();
    expect(await screen.findByText(/voice is turned off/i)).toBeInTheDocument();
  });

  it("cleans up what Windows typed", () => {
    expect(spokenQuery("  Open Spotify. ")).toBe("Open Spotify");
    expect(spokenQuery("What's 2 + 2?")).toBe("What's 2 + 2?");
    expect(spokenQuery("…")).toBe("…");
  });
});
