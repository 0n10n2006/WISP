import { fireEvent, render, screen, waitFor } from "@testing-library/preact";
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
    expect(screen.getByText(/nothing is sent until you do/i)).toBeInTheDocument();
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

  it("offers one-click actions on the circled area", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));
    await user.click(await screen.findByRole("button", { name: /search the web/i }));
    await waitFor(() => expect(ask).toHaveBeenCalledOnce());
    const req = ask.mock.calls[0]![0];
    expect(req.web).toBe(true);
    expect(req.contextKind).toBe("screen");
    expect(req.attachments).toHaveLength(1);
    expect(screen.queryByText(/screen area attached/i)).not.toBeInTheDocument();
  });

  it("reverse image searches the circled area with Google Lens", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const lens = vi.spyOn(h, "reverseImageSearch");
    const ask = vi.spyOn(h, "ask");
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));
    await user.click(await screen.findByRole("button", { name: /google lens/i }));
    await waitFor(() => expect(lens).toHaveBeenCalledOnce());
    expect(lens.mock.calls[0]![0].length).toBeGreaterThan(10);
    expect(ask).not.toHaveBeenCalled();
    expect(await screen.findByText(/google lens/i)).toBeInTheDocument();
    expect(screen.queryByText(/screen area attached/i)).not.toBeInTheDocument();
  });

  it("copies the text in the circled area", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const copy = vi.spyOn(h, "copyText").mockResolvedValue();
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: /open wisp/i }));
    await user.click(await screen.findByRole("button", { name: /select an area/i }));
    await user.click(await screen.findByRole("button", { name: /copy text/i }));
    await waitFor(() => expect(copy).toHaveBeenCalled(), { timeout: 5000 });
    expect(await screen.findByText(/text copied/i)).toBeInTheDocument();
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
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
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
    fireEvent.click(screen.getByRole("radio", { name: /box/i }));

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

describe("lasso", () => {
  it("circles an area and sends the path, not just a box", async () => {
    try {
      localStorage.removeItem("wisp.overlay.mode");
    } catch {
      /* ignore */
    }
    const h = await setupMockHost({ windowKind: "overlay" });
    const lasso = vi.spyOn(h, "captureSelectLasso");
    render(<App />);
    const overlay = await screen.findByRole("application");
    expect(screen.getByRole("radio", { name: /circle/i })).toHaveAttribute("aria-checked", "true");
    fireEvent.pointerDown(overlay, { button: 0, clientX: 100, clientY: 100 });
    for (const [x, y] of [
      [200, 90],
      [260, 160],
      [200, 240],
      [110, 200],
    ])
      fireEvent.pointerMove(overlay, { clientX: x, clientY: y });
    expect(screen.getByTestId("lasso")).toBeInTheDocument();
    fireEvent.pointerUp(overlay);
    await waitFor(() => expect(lasso).toHaveBeenCalledOnce());
    expect(lasso.mock.calls[0]![1]).toHaveLength(5);
  });
});

describe("seamless overlay", () => {
  it("asks to be shown only after its frozen screen is painted", async () => {
    const h = await setupMockHost({ windowKind: "overlay" });
    const ready = vi.spyOn(h, "captureOverlayReady");
    render(<App />);
    const overlay = await screen.findByRole("application");
    await waitFor(() => expect(overlay.style.backgroundImage).toContain("data:image"));
    await waitFor(() => expect(ready).toHaveBeenCalledOnce());
    expect(overlay.className).toContain("overlay-ready");
  });
});
