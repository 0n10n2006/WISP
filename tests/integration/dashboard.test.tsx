import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import type { AudioRecorder } from "../../src/services/recorder";
import { useConversation } from "../../src/state/conversation";
import { useDictation } from "../../src/state/dictation";
import { useNotch } from "../../src/state/notch";
import { useSettings } from "../../src/state/settings";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  await screen.findByRole("combobox", { name: /ask wisp/i });
  return user;
}

describe("notch modes", () => {
  it("peeks on hover and folds back to the sliver", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const mode = vi.spyOn(h, "setIslandMode");
    render(<App />);
    const notch = screen.getByRole("button", { name: /open wisp/i });
    fireEvent.pointerEnter(notch);
    await waitFor(() => expect(mode).toHaveBeenCalledWith("peek"));
    expect(await screen.findByText(/weightless/i)).toBeInTheDocument();
    fireEvent.pointerLeave(screen.getByRole("button", { name: /open wisp/i }));
    await waitFor(() => expect(mode).toHaveBeenCalledWith("sliver"), { timeout: 2000 });
  });

  it("folds away when clicking elsewhere, unless an answer is streaming", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const hide = vi.spyOn(h, "hideIsland");
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "explain /slow{Enter}");
    act(() => (window as unknown as { wispMock: { blur(): void } }).wispMock.blur());
    expect(hide).not.toHaveBeenCalled();
    await waitFor(() => expect(useConversation.getState().requestActive).toBe(false), {
      timeout: 6000,
    });
    act(() => (window as unknown as { wispMock: { blur(): void } }).wispMock.blur());
    expect(hide).toHaveBeenCalled();
  });
});

describe("command bar", () => {
  it("opens an app with Enter", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    // Without open windows, so launching isn't outranked by "switch to Spotify".
    await h.updateSettings({ appAwareness: false });
    await act(async () => {
      await useSettings.getState().load();
    });
    const launch = vi.spyOn(h, "launchItem");
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "spoti");
    const option = await screen.findByRole("option", { name: /spotify/i });
    expect(option).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(launch).toHaveBeenCalledWith(expect.objectContaining({ id: "lnk:spotify" })),
    );
  });

  it("starts a focus timer from a command", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "timer 10{Enter}");
    await waitFor(() => expect(useNotch.getState().timer?.minutes).toBe(10));
    expect(await screen.findByText(/focus timer: 10 min/i)).toBeInTheDocument();
  });

  it("confirms before shutting down", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const power = vi.spyOn(h, "powerAction");
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "shutdown{Enter}");
    const strip = await screen.findByRole("alertdialog", { name: /confirm/i });
    expect(power).not.toHaveBeenCalled();
    await user.click(within(strip).getByRole("button", { name: /shut down/i }));
    await waitFor(() => expect(power).toHaveBeenCalledWith("shutdown"));
  });

  it("asks with the web and shows sources; Shift+Enter thinks harder", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    const box = screen.getByRole("combobox", { name: /ask wisp/i });
    await user.type(box, "latest news about wisps?");
    await user.keyboard("{Tab}");
    expect(screen.getByRole("button", { name: /web/i })).toHaveAttribute("aria-pressed", "true");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
    expect(ask.mock.calls[0]![0]).toMatchObject({ web: true, think: false });
    expect(
      await screen.findByRole("button", { name: /wikipedia/i }, { timeout: 5000 }),
    ).toBeInTheDocument();
    await waitFor(() => expect(useConversation.getState().requestActive).toBe(false));

    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "prove it?");
    await user.keyboard("{Shift>}{Enter}{/Shift}");
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(2));
    expect(ask.mock.calls[1]![0]).toMatchObject({ think: true, web: false });
  });
});

describe("dashboard tabs", () => {
  it("shows every tab", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await openBar();
    const tabs = await screen.findByRole("tablist", { name: "Dashboard" });
    for (const [name, text] of [
      ["Controls", /now playing/i],
      ["Stats", /memory/i],
      ["Clipboard", /clipboard history is off/i],
      ["Apps", /dota 2/i],
      ["Agents", /research brief/i],
      ["Wallpaper", /browse/i],
      ["Power", /shut down/i],
      ["Today", /up next/i],
    ] as const) {
      await user.click(within(tabs).getByRole("tab", { name }));
      expect(await screen.findAllByText(text, {}, { timeout: 3000 })).not.toHaveLength(0);
    }
  });

  it("turns on clipboard history and copies an item again", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "clipCopy");
    const user = await openBar();
    await user.click(await screen.findByRole("tab", { name: "Clipboard" }));
    await user.click(await screen.findByRole("button", { name: /turn on clipboard history/i }));
    await waitFor(() =>
      expect(useSettings.getState().snapshot?.settings.clipboardHistory).toBe(true),
    );
    await user.click(await screen.findByRole("button", { name: /meeting moved to thursday/i }));
    expect(copy).toHaveBeenCalledWith("clip-2");
    await user.click(screen.getByRole("radio", { name: "Links" }));
    await waitFor(() => expect(screen.queryByText(/meeting moved/i)).not.toBeInTheDocument());
  });

  it("starts a research agent", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const start = vi.spyOn(h, "agentStart");
    const user = await openBar();
    await user.click(await screen.findByRole("tab", { name: "Agents" }));
    await user.type(await screen.findByRole("textbox", { name: /research topic/i }), "wisp lore");
    await user.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() =>
      expect(start).toHaveBeenCalledWith({ kind: "research", topic: "wisp lore" }),
    );
    expect(await screen.findByText("Research: wisp lore")).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Open" }, { timeout: 4000 }),
    ).toBeInTheDocument();
  });
});

describe("hold to talk", () => {
  function recorder(): AudioRecorder {
    return {
      start: vi.fn(async () => {}),
      stop: vi.fn(async () => ({ base64: "AAAA", mimeType: "audio/wav", durationMs: 1500 })),
      cancel: vi.fn(),
    };
  }

  it("dictates while Ctrl+Win is held and types it in on release", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    const insert = vi.spyOn(h, "insertText");
    render(<App />);
    const mock = (window as unknown as { wispMock: { hold(e: unknown): void } }).wispMock;
    act(() => mock.hold({ chord: "dictate", phase: "start" }));
    await waitFor(() => expect(useDictation.getState().phase).toBe("listening"));
    expect(await screen.findByText(/release to type it in/i)).toBeInTheDocument();
    act(() => mock.hold({ chord: "dictate", phase: "stop" }));
    await waitFor(() =>
      expect(insert).toHaveBeenCalledWith(
        "This is a simulated transcript from the browser preview.",
      ),
    );
  });

  it("discards the recording when the chord becomes a normal shortcut", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const rec = recorder();
    vi.spyOn(h, "createRecorder").mockReturnValue(rec);
    const transcribe = vi.spyOn(h, "transcribe");
    render(<App />);
    const mock = (window as unknown as { wispMock: { hold(e: unknown): void } }).wispMock;
    act(() => mock.hold({ chord: "dictate", phase: "start" }));
    await waitFor(() => expect(useDictation.getState().phase).toBe("listening"));
    act(() => mock.hold({ chord: "dictate", phase: "cancel" }));
    expect(rec.cancel).toHaveBeenCalled();
    expect(transcribe).not.toHaveBeenCalled();
  });
});

describe("racing hover and shortcut", () => {
  it("keeps the bar open when a hover arrives just after the shortcut opened it", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const { changeMode } = await import("../../src/screens/notch/control");
    // The shortcut opened the bar on the host, but the page hasn't heard yet.
    await h.setIslandMode("open");
    act(() => useNotch.setState({ mode: "sliver" }));
    await act(async () => {
      await changeMode("peek");
    });
    expect(useNotch.getState().mode).toBe("open");
  });
});
