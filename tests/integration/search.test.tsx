import { render, screen, waitFor, within } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useConfirm } from "../../src/screens/notch/actions";
import { useSettings } from "../../src/state/settings";
import { setupMockHost } from "../helpers";

async function openBar() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}
const box = () => screen.findByRole("combobox", { name: /ask wisp/i });
const results = () => screen.findByRole("listbox", { name: "Results" });
const first = async () => within(await results()).getAllByRole("option")[0]!;

describe("Fluent-style search in the bar", () => {
  it("a tag becomes a chip and scopes results; Backspace removes it", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await openBar();
    await user.type(await box(), "t:");
    const chip = await screen.findByRole("button", { name: /searching tabs/i });
    expect(chip).toBeInTheDocument();
    expect(await box()).toHaveValue("");
    await waitFor(async () =>
      expect(within(await results()).getAllByRole("option")).toHaveLength(3),
    );
    await user.type(await box(), "{Backspace}");
    expect(screen.queryByRole("button", { name: /searching tabs/i })).toBeNull();
  });

  it("Tab turns a tag word into the tag", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await openBar();
    await user.type(await box(), "processes{Tab}");
    expect(await screen.findByRole("button", { name: /searching processes/i })).toBeInTheDocument();
    // Tab still toggles Web for ordinary text.
    await user.type(await box(), "{Backspace}");
    await user.type(await box(), "weather{Tab}");
    expect(screen.getByRole("button", { name: "Web" })).toHaveAttribute("aria-pressed", "true");
  });

  it("switches to a browser tab", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const sw = vi.spyOn(h, "browserTabSwitch");
    const user = await openBar();
    await user.type(await box(), "t: inbox");
    await waitFor(async () => expect(await first()).toHaveTextContent("Inbox (3) - Gmail"));
    await user.keyboard("{Enter}");
    await waitFor(() => expect(sw).toHaveBeenCalledWith(1));
  });

  it("opens a page from history by id", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const open = vi.spyOn(h, "browserOpen");
    const user = await openBar();
    await user.type(await box(), "h: hacker");
    await waitFor(async () => expect(await first()).toHaveTextContent("Hacker News"));
    await user.keyboard("{Enter}");
    await waitFor(() => expect(open).toHaveBeenCalledWith(12));
  });

  it("ends a program after asking; Windows' own are refused", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const end = vi.spyOn(h, "processEnd");
    const user = await openBar();
    await user.type(await box(), "p: notepad");
    await waitFor(async () => expect(await first()).toHaveTextContent("notepad"));
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(useConfirm.getState().pending?.label).toBe("End notepad"));
    expect(end).not.toHaveBeenCalled();
    await useConfirm.getState().pending!.run();
    expect(end).toHaveBeenCalledWith("notepad");

    await user.clear(await box());
    await user.type(await box(), "csrss");
    await waitFor(async () => expect(await first()).toHaveTextContent("csrss"));
    useConfirm.getState().clear();
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(await screen.findByText(/won't end csrss/i)).toBeInTheDocument();
    expect(end).toHaveBeenCalledTimes(1);
  });

  it("calculates as you type and Enter copies", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "copyText");
    const ask = vi.spyOn(h, "ask");
    const user = await openBar();
    await user.type(await box(), "12*7");
    expect(await first()).toHaveTextContent("= 84");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(copy).toHaveBeenCalledWith("84"));
    expect(ask).not.toHaveBeenCalled();
  });

  it("converts currency with the day's rates", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const rates = vi.spyOn(h, "currencyRates");
    const user = await openBar();
    await user.type(await box(), "10 eur to usd");
    await waitFor(async () => expect(await first()).toHaveTextContent("= 10.80 USD"));
    expect(rates).toHaveBeenCalledTimes(1);
  });

  it("“g cats” opens a Google search", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ext = vi.spyOn(h, "openExternal");
    const user = await openBar();
    await user.type(await box(), "g cats");
    expect(await first()).toHaveTextContent("Search Google for “cats”");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(ext).toHaveBeenCalledWith("https://www.google.com/search?q=cats"));
  });

  it("previews the selected file", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = await openBar();
    await user.type(await box(), "f: holiday");
    const pane = await screen.findByRole("complementary", { name: /preview of holiday photos/i });
    await waitFor(() => expect(within(pane).getByText("IMG_2041.jpg")).toBeInTheDocument());
  });

  it("no tabs or history when browser search is off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await useSettings.getState().update({ browserSearch: false });
    const tabs = vi.spyOn(h, "browserTabs");
    const search = vi.spyOn(h, "browserSearch");
    const user = await openBar();
    await user.type(await box(), "t: inbox");
    await new Promise((r) => setTimeout(r, 200));
    expect(tabs).not.toHaveBeenCalled();
    expect(search).not.toHaveBeenCalled();
    expect(screen.queryByText("Inbox (3) - Gmail")).toBeNull();
  });
});

describe("Settings → Search", () => {
  it("adds and removes web shortcuts and lists browsers", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Search" }));
    expect(await screen.findByText(/Found: Chrome, Edge, Firefox/)).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Web search shortcuts" });
    expect(within(list).getByText("Google")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Shortcut"), "npm");
    await user.type(screen.getByLabelText("Name"), "npm registry");
    await user.type(
      screen.getByLabelText(/address, with %s/i),
      "https://www.npmjs.com/search?q=%s",
    );
    await user.click(screen.getByRole("button", { name: /add shortcut/i }));
    expect(await within(list).findByText("npm registry")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove Google" }));
    await waitFor(() => expect(within(list).queryByText("Google")).toBeNull());
    await user.click(screen.getByRole("button", { name: /reset to defaults/i }));
    expect(await within(list).findByText("Google")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: /search browser tabs/i }));
    await waitFor(() =>
      expect(useSettings.getState().snapshot?.settings.browserSearch).toBe(false),
    );
  });
});
