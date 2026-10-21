import { act, render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useClipboardAssist, useDueReminders } from "../../src/state/assist";
import { setupMockHost } from "../helpers";

async function island() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

describe("model-proposed actions", () => {
  it("asks before opening a link and records the outcome", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await island();
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "open example /open{Enter}",
    );
    const group = await screen.findByRole("group", { name: /asking for permission/i });
    expect(group).toHaveTextContent("Open https://example.com in your browser");
    await user.click(within(group).getByRole("button", { name: "Allow" }));
    expect(await screen.findByText(/opened the link/i)).toBeInTheDocument();
    const audit = await h.actionsList();
    expect(audit[0]).toMatchObject({
      actionType: "open_url",
      outcome: "done",
      userConfirmed: true,
    });
  });

  it("does nothing when the user declines", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await island();
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "remember that /remember{Enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Don't allow" }));
    expect(await screen.findByText(/you chose not to do this/i)).toBeInTheDocument();
    expect(await h.memoryList()).toHaveLength(0);
    expect((await h.actionsList())[0]).toMatchObject({ outcome: "declined" });
  });

  it("saves a memory only after approval", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await island();
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "remember /remember{Enter}",
    );
    await user.click(await screen.findByRole("button", { name: "Allow" }));
    await waitFor(async () => expect(await h.memoryList()).toHaveLength(1));
  });
});

describe("clipboard help", () => {
  beforeEach(() => useClipboardAssist.getState().close());

  it("shows what will be sent and sends it with a short label", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "clipboardRead").mockResolvedValue("Some copied paragraph.");
    const ask = vi.spyOn(h, "ask");
    const user = await island();
    await user.click(await screen.findByRole("button", { name: "Clipboard help" }));
    expect(await screen.findByLabelText("Clipboard text")).toHaveTextContent(
      "Some copied paragraph.",
    );
    expect(screen.getByText(/22 characters will be sent/i)).toBeInTheDocument();
    expect(ask).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Summarize" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
    expect(ask.mock.calls[0]![0].contextKind).toBe("clipboard");
    expect(ask.mock.calls[0]![0].prompt).toContain("Some copied paragraph.");
    expect(
      await screen.findByText(/summarize — clipboard text \(22 characters\)/i),
    ).toBeInTheDocument();
  });

  it("handles a clipboard without text", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "clipboardRead").mockResolvedValue(null);
    const user = await island();
    await user.click(await screen.findByRole("button", { name: "Clipboard help" }));
    expect(await screen.findByText(/doesn't contain text/i)).toBeInTheDocument();
  });
});

describe("reminders", () => {
  beforeEach(() => useDueReminders.setState({ due: [] }));

  it("creates, completes and deletes reminders", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Reminders" }));
    await user.type(screen.getByLabelText("Title"), "Water the plants");
    await user.click(screen.getByRole("button", { name: /add reminder/i }));
    const list = await screen.findByRole("list", { name: "Reminders" });
    expect(within(list).getByText("Water the plants")).toBeInTheDocument();

    await user.click(within(list).getByRole("checkbox", { name: /mark "water the plants" done/i }));
    await waitFor(() =>
      expect(
        within(screen.queryByRole("list", { name: "Reminders" }) ?? document.body).queryByText(
          "Water the plants",
        ),
      ).toBeNull(),
    );
    await user.click(screen.getByRole("checkbox", { name: /show completed/i }));
    const all = await screen.findByRole("list", { name: "Reminders" });
    await user.click(within(all).getByRole("button", { name: /delete "water the plants"/i }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Delete" }),
    );
    await waitFor(async () => expect(await h.tasksList(true)).toHaveLength(0));
  });

  it("rejects reminders in the past", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Reminders" }));
    await user.type(screen.getByLabelText("Title"), "Too late");
    const when = screen.getByLabelText("When");
    await user.clear(when);
    await user.type(when, "2020-01-01T10:00");
    await user.click(screen.getByRole("button", { name: /add reminder/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/in the past/i);
  });

  it("shows due reminders in the island with Done and Snooze", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const t = await h.taskCreate({
      title: "Stand up",
      dueAt: new Date(Date.now() + 60_000).toISOString(),
    });
    const complete = vi.spyOn(h, "taskComplete");
    const user = await island();
    act(() => useDueReminders.getState().push(t));
    const banner = await screen.findByText("Reminder: Stand up");
    expect(banner).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(complete).toHaveBeenCalledWith(t.id, true);
    expect(screen.queryByText("Reminder: Stand up")).not.toBeInTheDocument();
  });

  it("creates reminders when the model asks", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await island();
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "remind me /remind{Enter}",
    );
    expect(await screen.findByText(/reminder created/i)).toBeInTheDocument();
    expect(await h.tasksList(false)).toHaveLength(1);
  });
});

describe("memory & actions settings", () => {
  it("adds, disables and deletes memories", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Memory" }));
    await user.type(screen.getByLabelText("New memory"), "I live in Leeds");
    await user.click(screen.getByRole("button", { name: "Add" }));
    const list = await screen.findByRole("list", { name: "Memories" });
    await user.click(within(list).getByRole("switch", { name: /use "i live in leeds"/i }));
    await waitFor(async () => expect((await h.memoryList())[0]!.enabled).toBe(false));
    await user.click(
      within(await screen.findByRole("list", { name: "Memories" })).getByRole("button", {
        name: /delete memory/i,
      }),
    );
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Delete" }),
    );
    await waitFor(async () => expect(await h.memoryList()).toHaveLength(0));
  });

  it("refuses shells in the app allowlist", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Actions" }));
    await user.type(screen.getByLabelText(/name wisp should use/i), "terminal");
    await user.type(screen.getByLabelText(/program or shortcut/i), "cmd.exe");
    await user.click(screen.getByRole("button", { name: "Add app" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/shells and script hosts/i);
    expect(screen.getByLabelText(/name wisp should use/i)).toHaveValue("terminal");

    await user.clear(screen.getByLabelText(/program or shortcut/i));
    await user.type(screen.getByLabelText(/program or shortcut/i), "wt.exe");
    await user.click(screen.getByRole("button", { name: "Add app" }));
    expect(
      await within(screen.getByRole("list", { name: "Allowed apps" })).findByText("terminal"),
    ).toBeInTheDocument();
  });
});
