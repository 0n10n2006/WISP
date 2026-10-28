import { act, render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useConversation } from "../../src/state/conversation";
import { useSettings } from "../../src/state/settings";
import { useVoice } from "../../src/state/voice";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

const box = () => screen.findByRole("combobox", { name: /ask wisp/i });

describe("instant file search", () => {
  it("shows matching files as you type; Enter opens by id", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const open = vi.spyOn(h, "fileOpen");
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.type(await box(), "resu");
    const list = await screen.findByRole("listbox", { name: "Results" });
    await waitFor(() =>
      expect(within(list).getAllByRole("option")[0]).toHaveTextContent("Resume.pdf"),
    );
    expect(within(list).getByText("Resume 2023 old.docx")).toBeInTheDocument();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(open).toHaveBeenCalledWith(1));
    expect(ask).not.toHaveBeenCalled();
  });

  it("Ctrl+Enter shows the file in its folder", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const reveal = vi.spyOn(h, "fileReveal");
    const user = await openBar();
    await user.type(await box(), "find invoice");
    await screen.findByText("Invoice_March.pdf");
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(reveal).toHaveBeenCalledWith(3));
  });

  it("“open my …” opens the file straight away", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const open = vi.spyOn(h, "fileOpen");
    const user = await openBar();
    await user.type(await box(), "open my budget");
    await screen.findByText("budget.xlsx");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(open).toHaveBeenCalledWith(4));
  });

  it("saying “open my resume” opens it with no model call", async () => {
    const h = await setupMockHost({
      withKey: true,
      onboarded: true,
      windowKind: "island",
      voice: "Open my resume.",
    });
    const open = vi.spyOn(h, "fileOpen");
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.click(await screen.findByRole("button", { name: "Ask by voice" }));
    await waitFor(() => expect(open).toHaveBeenCalledWith(1), { timeout: 3000 });
    expect(ask).not.toHaveBeenCalled();
    expect(useVoice.getState().listening).toBe(false);
  });

  it("no file rows when file search is off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await act(async () => void (await useSettings.getState().update({ indexFiles: false })));
    const search = vi.spyOn(h, "filesSearch");
    const user = await openBar();
    await user.type(await box(), "resume");
    await new Promise((r) => setTimeout(r, 150));
    expect(search).not.toHaveBeenCalled();
    expect(screen.queryByText("Resume.pdf")).not.toBeInTheDocument();
  });
});

describe("quick PC answers", () => {
  it("answers “battery” on this PC without asking Gemini", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.type(await box(), "battery{Enter}");
    await waitFor(() => expect(useConversation.getState().messages).toHaveLength(2));
    const [q, a] = useConversation.getState().messages;
    expect(q).toMatchObject({ role: "user", text: "battery" });
    expect(a).toMatchObject({ role: "model", note: "Answered on this PC" });
    expect(a!.text).toContain("76%");
    expect(ask).not.toHaveBeenCalled();
    expect(await screen.findByText("Answered on this PC")).toBeInTheDocument();
  });
});

describe("Search settings", () => {
  it("shows the indexed folders and lets you add and remove your own", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Search" }));
    expect(await screen.findByText(/8 items · updated 2 min ago/)).toBeInTheDocument();
    const folders = screen.getByRole("list", { name: "Indexed folders" });
    expect(within(folders).getByText("Downloads")).toBeInTheDocument();
    // Standard folders can't be removed one by one.
    expect(screen.queryByRole("button", { name: /stop indexing downloads/i })).toBeNull();
    await user.click(screen.getByRole("button", { name: /add a folder/i }));
    expect(await within(folders).findByText("Projects")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /stop indexing projects/i }));
    await waitFor(() => expect(within(folders).queryByText("Projects")).toBeNull());
    await user.click(screen.getByRole("switch", { name: /index my files/i }));
    expect(await screen.findByText("File search is off.")).toBeInTheDocument();
  });
});
