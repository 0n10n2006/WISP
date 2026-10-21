import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useConversation } from "../../src/state/conversation";
import { useSettings } from "../../src/state/settings";
import { setupMockHost } from "../helpers";

describe("history & data", () => {
  it("logs metadata only by default and can clear history", async () => {
    const h = await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    await act(() => useConversation.getState().ask("private question"));
    await act(() => useConversation.getState().ask("again /quota"));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "History" }));
    const table = await screen.findByRole("table");
    expect(within(table).getAllByRole("row")).toHaveLength(3);
    expect(within(table).getByText("Answered")).toBeInTheDocument();
    expect(within(table).getByText("quota exhausted")).toBeInTheDocument();
    expect(screen.queryByText(/private question/)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /clear history/i }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: /clear history/i }),
    );
    await screen.findByText("No requests yet.");
    expect(await h.historyList()).toHaveLength(0);
  });

  it("stores content when the user opts in", async () => {
    const h = await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    await h.updateSettings({ contentHistory: true });
    await act(() => useConversation.getState().ask("remember this text"));
    expect((await h.historyList())[0]!.prompt).toBe("remember this text");
  });

  it("doesn't log when retention is zero", async () => {
    const h = await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    await h.updateSettings({ historyRetentionDays: 0 });
    await act(() => useConversation.getState().ask("hello"));
    expect(await h.historyList()).toHaveLength(0);
  });

  it("previews diagnostics without secrets and confirms before saving", async () => {
    const h = await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const save = vi.spyOn(h, "diagnosticsSave");
    await act(() => useConversation.getState().ask("secret prompt words"));
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Data" }));
    await user.click(screen.getByRole("button", { name: /preview diagnostics/i }));
    const pre = await screen.findByLabelText(/diagnostics preview/i);
    expect(pre.textContent).toContain('"hasApiKey": true');
    expect(pre.textContent).not.toContain("secret prompt");
    await user.click(screen.getByRole("button", { name: /save to downloads/i }));
    expect(save).not.toHaveBeenCalled();
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: /save file/i }),
    );
    expect(save).toHaveBeenCalledOnce();
    expect(await screen.findByText(/saved to/i)).toBeInTheDocument();
  });

  it("delete everything removes the key and restarts setup", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Data" }));
    await user.click(screen.getByRole("button", { name: /delete everything/i }));
    const dialog = screen.getByRole("alertdialog");
    expect(dialog).toHaveTextContent(/can't be undone/i);
    await user.click(within(dialog).getByRole("button", { name: /delete everything/i }));
    await waitFor(() => expect(useSettings.getState().snapshot?.hasApiKey).toBe(false));
    expect(await screen.findByRole("heading", { name: /meet wisp/i })).toBeInTheDocument();
  });
});
