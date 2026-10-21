import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { pickAlternative } from "../../src/components/ErrorBanner";
import { useConversation } from "../../src/state/conversation";
import { useSettings } from "../../src/state/settings";
import { MOCK_MODELS } from "../../src/services/mockHost";
import { setupMockHost } from "../helpers";

async function island() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

describe("island controls", () => {
  it("pins and unpins Wisp from the panel", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await island();
    const pin = await screen.findByRole("button", { name: /keep wisp on top/i });
    expect(pin).toHaveAttribute("aria-pressed", "true");
    await user.click(pin);
    await waitFor(() => expect(useSettings.getState().snapshot?.settings.alwaysOnTop).toBe(false));
    expect(screen.getByRole("button", { name: /keep wisp on top/i })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("lets a conversation opt out of memory", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = await island();
    await user.click(
      await screen.findByRole("button", { name: /use memory in this conversation/i }),
    );
    await user.type(screen.getByRole("textbox", { name: "Ask Wisp" }), "hi{Enter}");
    await waitFor(() => expect(ask).toHaveBeenCalled());
    expect(ask.mock.calls[0]![0].useMemory).toBe(false);
    // A new conversation turns memory back on.
    act(() => useConversation.getState().clear());
    expect(useConversation.getState().memoryForChat).toBe(true);
  });

  it("offers a working model when quota is used up and retries with it", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = await island();
    await user.type(await screen.findByRole("textbox", { name: "Ask Wisp" }), "test /quota{Enter}");
    const btn = await screen.findByRole("button", {
      name: /switch to gemini 2.5 flash-lite and retry/i,
    });
    await user.click(btn);
    await waitFor(() =>
      expect(useSettings.getState().snapshot?.settings.selectedModel).toBe("gemini-2.5-flash-lite"),
    );
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(2));
  });

  it("copies a single code block", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "copyText").mockResolvedValue();
    const user = await island();
    await user.type(await screen.findByRole("textbox", { name: "Ask Wisp" }), "code please{Enter}");
    await screen.findByRole("button", { name: "Copy" }, { timeout: 5000 });
    await user.click(screen.getByRole("button", { name: "Copy code" }));
    expect(copy).toHaveBeenCalledWith('const wisp = "quietly capable";');
  });
});

describe("pickAlternative", () => {
  it("prefers a non-lite Flash model and never returns the current one", () => {
    expect(pickAlternative(MOCK_MODELS, "gemini-2.5-pro")?.id).toBe("gemini-2.5-flash");
    expect(pickAlternative(MOCK_MODELS, "gemini-2.5-flash")?.id).toBe("gemini-2.5-flash-lite");
    expect(pickAlternative(MOCK_MODELS.slice(0, 1), "gemini-2.5-flash")).toBeNull();
  });
});

describe("settings additions", () => {
  it("adds an installed app from the picker", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Actions" }));
    await user.click(screen.getByRole("button", { name: /find installed apps/i }));
    await user.type(
      await screen.findByRole("searchbox", { name: /search installed apps/i }),
      "spot",
    );
    await user.click(screen.getByRole("button", { name: "Allow spotify" }));
    const allowed = await screen.findByRole("list", { name: "Allowed apps" });
    expect(within(allowed).getByText("spotify")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow spotify" })).not.toBeInTheDocument();
  });

  it("edits a reminder", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    await h.taskCreate({
      title: "Old title",
      dueAt: new Date(Date.now() + 3600_000).toISOString(),
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Reminders" }));
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    const form = screen.getByRole("form", { name: /edit "old title"/i });
    const title = within(form).getByLabelText("Title");
    await user.clear(title);
    await user.type(title, "New title");
    await user.click(within(form).getByRole("button", { name: "Save" }));
    await waitFor(async () => expect((await h.tasksList(false))[0]!.title).toBe("New title"));
  });

  it("changes Wisp's size and zooms the UI", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("radio", { name: "Largest" }));
    await waitFor(() =>
      expect(document.documentElement.style.getPropertyValue("zoom")).toBe("1.3"),
    );
    expect(useSettings.getState().snapshot?.settings.islandScale).toBe(1.3);
  });

  it("checks for updates only when asked", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const check = vi.spyOn(h, "checkForUpdates");
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "About" }));
    expect(check).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /check for updates/i }));
    expect(await screen.findByText(/latest version/i)).toBeInTheDocument();
  });
});

describe("regressions", () => {
  it("renders the Actions tab before settings have loaded", async () => {
    await setupMockHost({ windowKind: "settings" });
    useSettings.setState({ snapshot: null });
    const { ActionsTab } = await import("../../src/screens/settings/LocalTabs");
    render(<ActionsTab />);
    expect(screen.getByText("No apps allowed.")).toBeInTheDocument();
  });
});
