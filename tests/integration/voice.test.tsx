import { act, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { lastWords } from "../../src/screens/notch/Peek";
import { useNotch } from "../../src/state/notch";
import { useSettings } from "../../src/state/settings";
import { spokenQuery, useVoice } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

describe("voice (Windows speech recognition, no pop-up)", () => {
  it("dictation listens without opening the bar and shows the words in the notch", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "Meeting moved to Friday",
    });
    const start = vi.spyOn(h, "voiceStart");
    render(<App />);
    await act(() => useVoice.getState().dictate());
    expect(start).toHaveBeenCalledWith("app");
    expect(useNotch.getState().mode).toBe("peek");
    expect(await screen.findByText("Meeting moved to Friday")).toBeInTheDocument();
    // Listening ends by itself; the notch tucks back.
    await waitFor(() => expect(useVoice.getState().listening).toBe(false));
    await waitFor(() => expect(useNotch.getState().mode).toBe("sliver"));
  });

  it("the dictate button in the bar dictates into the app", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const start = vi.spyOn(h, "voiceStart");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Dictate" }));
    expect(start).toHaveBeenCalledWith("app");
  });

  it("asks by voice: the words fill the bar and are asked when you pause", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "What time is it in Tokyo.",
    });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    const box = screen.getByRole("combobox", { name: /ask wisp/i });
    await waitFor(() => expect(box).toHaveValue("What time is it"));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 3000 });
    // The full stop is dropped.
    expect(ask.mock.calls[0]![0].prompt).toBe("What time is it in Tokyo");
    expect(useVoice.getState().listening).toBe(false);
  });

  it("typing while it listens stops listening so you can edit", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "Summarise my whole week for me",
    });
    const ask = vi.spyOn(h, "ask");
    const cancel = vi.spyOn(h, "voiceCancel");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    const box = screen.getByRole("combobox", { name: /ask wisp/i });
    await waitFor(() => expect(box).toHaveValue("Summarise my"));
    await user.type(box, "!");
    expect(cancel).toHaveBeenCalled();
    expect(useVoice.getState().listening).toBe(false);
    await new Promise((r) => setTimeout(r, 600));
    expect(ask).not.toHaveBeenCalled();
    await user.type(box, "{Enter}");
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
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
    await waitFor(() => expect(useNotch.getState().timer).not.toBeNull(), { timeout: 3000 });
    expect(ask).not.toHaveBeenCalled();
  });

  it("says when voice is turned off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await act(async () => void (await useSettings.getState().update({ voiceEnabled: false })));
    const start = vi.spyOn(h, "voiceStart");
    render(<App />);
    await act(() => useVoice.getState().dictate());
    expect(start).not.toHaveBeenCalled();
    expect(await screen.findByText(/voice is turned off/i)).toBeInTheDocument();
  });

  it("explains when Windows speech recognition isn't available", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "voiceStart").mockRejectedValue({
      code: "permission_denied",
      message: 'Voice needs "Online speech recognition" turned on.',
      retryable: false,
    });
    render(<App />);
    await act(() => useVoice.getState().dictate());
    expect(await screen.findByText(/online speech recognition/i)).toBeInTheDocument();
    expect(useVoice.getState().listening).toBe(false);
    expect(useNotch.getState().mode).toBe("sliver");
  });

  it("tidies what was heard", () => {
    expect(spokenQuery("  Open Spotify. ")).toBe("Open Spotify");
    expect(spokenQuery("What's 2 + 2?")).toBe("What's 2 + 2?");
    expect(lastWords("short")).toBe("short");
    expect(lastWords("a".repeat(30) + " the newest words at the end", 20)).toBe(
      "…st words at the end",
    );
  });
});
