import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useConversation } from "../../src/state/conversation";
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

  it("control centre toggles Windows settings at once and rolls back if Windows refuses", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const set = vi.spyOn(h, "winSettingSet");
    const user = await openBar();
    await user.click(await screen.findByRole("tab", { name: "Controls" }));
    const night = await screen.findByRole("button", { name: /night light/i });
    expect(night).toHaveAttribute("aria-pressed", "false");
    await user.click(night);
    expect(night).toHaveAttribute("aria-pressed", "true");
    expect(set).toHaveBeenCalledWith({ setting: "nightLight", value: true });

    set.mockRejectedValueOnce({ code: "unsupported", message: "Not on this PC", retryable: false });
    const dnd = screen.getByRole("button", { name: /do not disturb/i });
    await user.click(dnd);
    await waitFor(() => expect(dnd).toHaveAttribute("aria-pressed", "false"));
    expect(await screen.findByText("Not on this PC")).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /headphones/i }));
    expect(set).toHaveBeenCalledWith({ setting: "outputDevice", value: "hp" });
    await user.click(screen.getByRole("radio", { name: "Left" }));
    expect(set).toHaveBeenCalledWith({ setting: "taskbarAlign", value: "left" });
    await user.click(screen.getByRole("radio", { name: "Red" }));
    expect(set).toHaveBeenCalledWith({ setting: "accentColor", value: "#e81123" });
    await user.click(screen.getByRole("button", { name: /power mode/i }));
    expect(set).toHaveBeenCalledWith({ setting: "powerMode", value: "performance" });
  });

  it("flips through wallpaper folders", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const setWall = vi.spyOn(h, "wallpaperSet");
    const next = vi.spyOn(h, "wallpaperNext");
    const user = await openBar();
    await user.click(await screen.findByRole("tab", { name: "Wallpaper" }));
    await user.click(await screen.findByRole("button", { name: /add folder/i }));
    const strip = await screen.findByRole("list", { name: "Wallpapers" });
    await waitFor(() => expect(within(strip).getAllByRole("button")).toHaveLength(6));
    await user.click(within(strip).getAllByRole("button")[2]!);
    expect(setWall).toHaveBeenCalledWith(expect.stringMatching(/wallpaper-3\.jpg$/));
    await waitFor(() =>
      expect(within(strip).getAllByRole("button")[2]).toHaveAttribute("aria-pressed", "true"),
    );
    await user.click(screen.getByRole("button", { name: /^next$/i }));
    expect(next).toHaveBeenCalledWith(false);
    await waitFor(() =>
      expect(within(strip).getAllByRole("button")[3]).toHaveAttribute("aria-pressed", "true"),
    );
    await user.click(screen.getByRole("button", { name: /remove folder/i }));
    // Without folders the strip falls back to recently used wallpapers.
    await waitFor(() =>
      expect(
        within(screen.getByRole("list", { name: "Wallpapers" })).getAllByRole("button"),
      ).toHaveLength(1),
    );
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

describe("voice keys", () => {
  const mock = () => (window as unknown as { wispMock: { hold(e: unknown): void } }).wispMock;

  it("tapping Ctrl+Win starts listening for dictation into the app you're in", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const typing = vi.spyOn(h, "voiceStart");
    render(<App />);
    act(() => mock().hold({ chord: "dictate", phase: "tap" }));
    await waitFor(() => expect(typing).toHaveBeenCalledWith("app"));
    expect(useNotch.getState().mode).not.toBe("open");
    // Tapping again stops.
    const stop = vi.spyOn(h, "voiceStop");
    act(() => mock().hold({ chord: "dictate", phase: "tap" }));
    await waitFor(() => expect(stop).toHaveBeenCalled());
  });

  it("tapping Win+Alt opens the bar and listens for a question", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const typing = vi.spyOn(h, "voiceStart");
    render(<App />);
    act(() => mock().hold({ chord: "ask", phase: "tap" }));
    await waitFor(() => expect(typing).toHaveBeenCalledWith("bar"));
    expect(useNotch.getState().mode).toBe("open");
    expect(await screen.findByPlaceholderText("Speak now…")).toBeInTheDocument();
  });

  it("a held chord starts on release; a chord that became a shortcut does nothing", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const typing = vi.spyOn(h, "voiceStart");
    render(<App />);
    act(() => mock().hold({ chord: "dictate", phase: "start" }));
    act(() => mock().hold({ chord: "dictate", phase: "cancel" }));
    await new Promise((r) => setTimeout(r, 50));
    expect(typing).not.toHaveBeenCalled();
    act(() => mock().hold({ chord: "dictate", phase: "start" }));
    act(() => mock().hold({ chord: "dictate", phase: "stop" }));
    await waitFor(() => expect(typing).toHaveBeenCalledOnce());
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
