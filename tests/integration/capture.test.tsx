import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { DEFAULT_SCREEN_PROMPT, useCapture } from "../../src/state/capture";
import { setupMockHost } from "../helpers";

describe("select to ask", () => {
  beforeEach(() => useCapture.setState({ pending: null }));

  it("attaches the selected area and sends it only on confirmation", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));

    expect(await screen.findByText(/screen area attached/i)).toBeInTheDocument();
    expect(screen.getByText(/only when you press send/i)).toBeInTheDocument();
    expect(ask).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
    const req = ask.mock.calls[0]![0];
    expect(req.contextKind).toBe("screen");
    expect(req.attachments).toHaveLength(1);
    expect(req.prompt).toBe(DEFAULT_SCREEN_PROMPT);
    expect(await screen.findByAltText("Selected screen region")).toBeInTheDocument();
    expect(screen.queryByText(/screen area attached/i)).not.toBeInTheDocument();
  });

  it("can discard the capture", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));
    await user.click(await screen.findByRole("button", { name: /remove screen area/i }));
    expect(screen.queryByText(/screen area attached/i)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    expect(ask).not.toHaveBeenCalled();
  });

  it("explains when screen capture is turned off", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    await h.updateSettings({ screenContextEnabled: false });
    const open = vi.spyOn(h, "openSettings");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/turned off/i);
    await user.click(screen.getByRole("button", { name: /open settings/i }));
    expect(open).toHaveBeenCalledWith("privacy");
  });
});

describe("selection overlay", () => {
  it("selects by dragging, ignores tiny drags, and cancels with Escape", async () => {
    const h = await setupMockHost({ windowKind: "overlay" });
    const select = vi.spyOn(h, "captureSelect");
    const cancel = vi.spyOn(h, "captureCancel");
    render(<App />);
    const overlay = await screen.findByRole("application");
    await waitFor(() => expect(overlay.style.backgroundImage).toContain("data:image"));

    fireEvent.pointerDown(overlay, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(overlay, { clientX: 12, clientY: 12 });
    fireEvent.pointerUp(overlay);
    expect(select).not.toHaveBeenCalled();

    fireEvent.pointerDown(overlay, { button: 0, clientX: 300, clientY: 200 });
    fireEvent.pointerMove(overlay, { clientX: 100, clientY: 50 });
    expect(screen.getByTestId("selection")).toHaveStyle({
      left: "100px",
      top: "50px",
      width: "200px",
    });
    fireEvent.pointerUp(overlay);
    await waitFor(() => expect(select).toHaveBeenCalledOnce());
    expect(select.mock.calls[0]![1]).toMatchObject({ x: 100, y: 50, width: 200, height: 150 });

    fireEvent.keyDown(overlay, { key: "Escape" });
    expect(cancel).toHaveBeenCalled();
  });

  it("supports keyboard-only whole-screen selection", async () => {
    const h = await setupMockHost({ windowKind: "overlay" });
    const select = vi.spyOn(h, "captureSelect");
    render(<App />);
    fireEvent.keyDown(await screen.findByRole("application"), { key: "Enter" });
    await waitFor(() => expect(select).toHaveBeenCalledOnce());
    expect(select.mock.calls[0]![1]).toMatchObject({ x: 0, y: 0 });
  });
});
