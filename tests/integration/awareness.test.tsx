import { act, render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { createFakeRecorder } from "../../src/services/recorder";
import { useConversation } from "../../src/state/conversation";
import { useNotch } from "../../src/state/notch";
import { useSettings } from "../../src/state/settings";
import { useVoice } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  await screen.findByRole("combobox", { name: /ask wisp/i });
  return user;
}

async function reload() {
  await act(async () => {
    await useSettings.getState().load();
  });
}

describe("open windows", () => {
  it("switches to an open window from the bar", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const sw = vi.spyOn(h, "switchWindow");
    const launch = vi.spyOn(h, "launchItem");
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "switch to code");
    const option = await screen.findByRole("option", { name: /visual studio code/i });
    expect(option).toHaveTextContent(/switch to code/i);
    await user.keyboard("{Enter}");
    await waitFor(() => expect(sw).toHaveBeenCalledWith(2));
    expect(launch).not.toHaveBeenCalled();
  });

  it("doesn't look at windows when app awareness is off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ appAwareness: false });
    await reload();
    const list = vi.spyOn(h, "openWindows");
    const user = await openBar();
    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "spotify");
    const options = await screen.findAllByRole("option", { name: /spotify/i });
    expect(options.length).toBeGreaterThan(0);
    for (const o of options) expect(o).not.toHaveTextContent(/switch to/i);
    expect(list).not.toHaveBeenCalled();
  });
});

describe("customisation", () => {
  it("shows only the chosen dashboard tabs, in order", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ dashboardTabs: ["apps", "today"] });
    await reload();
    useNotch.setState({ tab: "stats" });
    await openBar();
    const tabs = within(await screen.findByRole("tablist", { name: "Dashboard" }));
    expect(tabs.getAllByRole("tab").map((t) => t.textContent)).toEqual(["Apps", "Today"]);
    // The hidden tab falls back to the first one shown.
    expect(tabs.getByRole("tab", { name: "Apps" })).toHaveAttribute("aria-selected", "true");
  });

  it("keeps the bar open on blur when asked to", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ closeOnBlur: false });
    await reload();
    const hide = vi.spyOn(h, "hideIsland");
    await openBar();
    act(() => (window as unknown as { wispMock: { blur(): void } }).wispMock.blur());
    expect(hide).not.toHaveBeenCalled();
  });

  it("edits dashboard tabs, position and the never-over list in Settings", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Notch & personality" }));
    await user.click(screen.getByRole("radio", { name: "Right" }));
    await user.click(screen.getByRole("checkbox", { name: "Wallpaper" }));
    await user.click(screen.getByRole("button", { name: "Move Power earlier" }));
    await waitFor(() => {
      const s = useSettings.getState().snapshot!.settings;
      expect(s.notchPosition).toBe("right");
      expect(s.dashboardTabs).toEqual([
        "today",
        "controls",
        "stats",
        "clipboard",
        "apps",
        "power",
        "agents",
      ]);
    });

    await user.click(screen.getByRole("tab", { name: "Games & focus" }));
    expect(await screen.findByText(/showing: nothing in front/i)).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: /app to add/i }), "C:\\Tools\\OBS64.exe");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(await screen.findByRole("button", { name: "+ spotify" }));
    await waitFor(() =>
      expect(useSettings.getState().snapshot!.settings.neverOverApps).toEqual(["obs64", "spotify"]),
    );
    await user.click(screen.getByRole("button", { name: "Remove obs64" }));
    await waitFor(() =>
      expect(useSettings.getState().snapshot!.settings.neverOverApps).toEqual(["spotify"]),
    );
  });
});

describe("knowing when you're done talking", () => {
  beforeEach(() => useVoice.getState().cancel());

  it("asks by itself after you pause", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ silenceMs: 500 });
    await reload();
    vi.spyOn(h, "createRecorder").mockReturnValue(createFakeRecorder(300));
    const ask = vi.spyOn(h, "ask");
    await act(() => useVoice.getState().start());
    expect(useVoice.getState().handsFree).toBe(true);
    await waitFor(() => expect(ask).toHaveBeenCalledOnce(), { timeout: 4000 });
    expect(ask.mock.calls[0]![0]).toMatchObject({ contextKind: "voice" });
    await waitFor(() => expect(useConversation.getState().requestActive).toBe(false), {
      timeout: 6000,
    });
  });

  it("waits for the key when holding to talk", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ silenceMs: 500 });
    await reload();
    vi.spyOn(h, "createRecorder").mockReturnValue(createFakeRecorder(100));
    const ask = vi.spyOn(h, "ask");
    await act(() => useVoice.getState().start(true));
    expect(useVoice.getState().handsFree).toBe(false);
    await new Promise((r) => setTimeout(r, 1000));
    expect(ask).not.toHaveBeenCalled();
    expect(useVoice.getState().phase).toBe("listening");
    useVoice.getState().cancel();
  });
});
