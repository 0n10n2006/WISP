import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { setupMockHost } from "../helpers";

describe("island", () => {
  it("expands from the orb, answers, copies, and collapses with Esc keeping the draft", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "copyText").mockResolvedValue();
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    const box = await screen.findByRole("textbox", { name: /ask wisp/i });
    expect(box).toHaveFocus();

    await user.type(box, "What is a wisp?{Enter}");
    await screen.findByText(/simulated answer/i);
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy" })).toBeInTheDocument(), {
      timeout: 5000,
    });
    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(copy).toHaveBeenCalledWith(expect.stringContaining("What is a wisp?"));
    expect(await screen.findByText("Copied")).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: /ask wisp/i }), "unsent thought");
    await user.keyboard("{Escape}");
    await screen.findByRole("button", { name: /open wisp/i });
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    expect(await screen.findByRole("textbox", { name: /ask wisp/i })).toHaveValue("unsent thought");
  });

  it("shows an actionable quota banner and keeps the prompt", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.type(
      await screen.findByRole("textbox", { name: /ask wisp/i }),
      "test /quota{Enter}",
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/quota used up/i);
    expect(alert).toHaveTextContent(/try again in about 3 h/i);
    expect(screen.getByRole("button", { name: /switch model/i })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: /ask wisp/i })).toHaveValue("test /quota");
    expect(screen.getByText("Needs attention")).toBeInTheDocument();
  });

  it("prompts for setup when no key is saved", async () => {
    await setupMockHost({ windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    expect(await screen.findByText(/connect gemini to start/i)).toBeInTheDocument();
  });

  it("can stop a running answer", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.type(await screen.findByRole("textbox", { name: /ask wisp/i }), "wait /slow{Enter}");
    expect(screen.getByText("Thinking…", { selector: ".status" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /stop answer/i }));
    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: /ask wisp/i })).toHaveValue("wait /slow"),
    );
  });
});
