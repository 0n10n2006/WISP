import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { setupMockHost } from "../helpers";

describe("notch", () => {
  it("opens from the sliver, answers, copies, and folds away with Esc keeping the draft", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "copyText").mockResolvedValue();
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("combobox", { name: /ask wisp/i });
    await waitFor(() => expect(box).toHaveFocus());

    await user.type(box, "What is a wisp?{Enter}");
    await screen.findByText(/simulated answer/i);
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(copy).toHaveBeenCalledWith(expect.stringContaining("What is a wisp?"));
    expect(await screen.findByText("Copied")).toBeInTheDocument();

    await user.type(screen.getByRole("combobox", { name: /ask wisp/i }), "unsent thought");
    await user.keyboard("{Escape}");
    await screen.findByRole("button", { name: /open wisp/i });
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    expect(await screen.findByRole("combobox", { name: /ask wisp/i })).toHaveValue(
      "unsent thought",
    );
  });

  it("shows an actionable quota banner and keeps the prompt", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "test /quota{Enter}",
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/quota used up/i);
    expect(alert).toHaveTextContent(/try again in about 3 h/i);
    expect(screen.getByRole("button", { name: /switch model/i })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: /ask wisp/i })).toHaveValue("test /quota");
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
  });

  it("prompts for setup when no key is saved", async () => {
    await setupMockHost({ windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    expect(await screen.findByText(/connect gemini to ask questions/i)).toBeInTheDocument();
  });

  it("can stop a running answer", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.type(
      await screen.findByRole("combobox", { name: /ask wisp/i }),
      "wait /slow{Enter}",
    );
    expect(screen.getAllByText("Thinking…").length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: /stop answer/i }));
    await waitFor(() =>
      expect(screen.getByRole("combobox", { name: /ask wisp/i })).toHaveValue("wait /slow"),
    );
  });

  it("folds away when the mouse leaves, but not while you're typing", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const hide = vi.spyOn(h, "hideIsland");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("combobox", { name: /ask wisp/i });
    const wrap = () => document.querySelector(".bar-wrap")!;

    // Typing just now: leaving doesn't fold.
    await user.type(box, "half a thought");
    fireEvent.pointerLeave(wrap());
    await new Promise((r) => setTimeout(r, 1200));
    expect(hide).not.toHaveBeenCalled();

    // Coming back cancels a pending fold.
    await user.clear(box);
    fireEvent.pointerLeave(wrap());
    fireEvent.pointerEnter(wrap());
    await new Promise((r) => setTimeout(r, 1200));
    expect(hide).not.toHaveBeenCalled();

    fireEvent.pointerLeave(wrap());
    await waitFor(() => expect(hide).toHaveBeenCalled(), { timeout: 3000 });
    expect(await screen.findByRole("button", { name: /open wisp/i })).toBeInTheDocument();
  });

  it("stays open on leave when folding on leave is off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ foldOnLeave: false });
    const hide = vi.spyOn(h, "hideIsland");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await screen.findByRole("combobox", { name: /ask wisp/i });
    fireEvent.pointerLeave(document.querySelector(".bar-wrap")!);
    await new Promise((r) => setTimeout(r, 1200));
    expect(hide).not.toHaveBeenCalled();
  });

  it("folds with Esc even when focus isn't in the bar, and shows how", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("combobox", { name: /ask wisp/i });
    expect(screen.getByText(/esc or ctrl\+alt\+space to fold away/i)).toBeInTheDocument();
    await waitFor(() => expect(box).toHaveFocus());
    box.blur();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(await screen.findByRole("button", { name: /open wisp/i })).toBeInTheDocument();
  });

  it("can rest as a floating ball in a corner and opens from it", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ notchStyle: "ball", ballCorner: "top-left" });
    const user = userEvent.setup();
    render(<App />);
    const btn = screen.getByRole("button", { name: /open wisp/i });
    await waitFor(() => expect(document.documentElement.dataset.notch).toBe("ball"));
    expect(document.documentElement.dataset.corner).toBe("top-left");
    expect(btn.querySelector(".ball")).not.toBeNull();
    expect(btn.querySelector(".sliver")).toBeNull();
    await user.click(btn);
    expect(await screen.findByRole("combobox", { name: /ask wisp/i })).toBeInTheDocument();
  });

  it("says when another free model answered because the first was used up", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("combobox", { name: /ask wisp/i });
    await user.type(box, "hello /fallback{Enter}");
    expect(
      await screen.findByText(/answered by gemini 2\.5 flash lite/i, undefined, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(await screen.findByText(/gemini 2\.5 flash is used up or busy/i)).toBeInTheDocument();
  });

  it("can approve an action for later", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("combobox", { name: /ask wisp/i });
    await user.type(box, "open chrome in 20 seconds /later{Enter}");
    await user.click(await screen.findByRole("button", { name: "Allow" }));
    expect(await screen.findByText(/it will run in 20 seconds/i)).toBeInTheDocument();
    expect(screen.getByText("Scheduled:")).toBeInTheDocument();
  });
});
