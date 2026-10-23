import { act, render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import type { AudioRecorder } from "../../src/services/recorder";
import { useActivity } from "../../src/state/activity";
import { useDictation } from "../../src/state/dictation";
import { setupMockHost } from "../helpers";

function recorder(overrides: Partial<AudioRecorder> = {}): AudioRecorder {
  return {
    start: vi.fn(async (onLevel: (l: number) => void) => onLevel(0.5)),
    stop: vi.fn(async () => ({ base64: "AAAA", mimeType: "audio/webm", durationMs: 2000 })),
    cancel: vi.fn(),
    ...overrides,
  };
}

async function openIsland() {
  const user = userEvent.setup();
  render(<App />);
  await user.click(screen.getByRole("button", { name: /open wisp/i }));
  return user;
}

describe("dictation", () => {
  beforeEach(() => {
    useDictation.getState().cancel();
    useActivity.setState({ listening: false });
  });

  it("records, shows an editable transcript and inserts the edited text", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const rec = recorder();
    vi.spyOn(h, "createRecorder").mockReturnValue(rec);
    const insert = vi.spyOn(h, "insertText");
    const user = await openIsland();

    await user.click(await screen.findByRole("button", { name: /^dictate/i }));
    expect(
      await screen.findByText("Listening…", { selector: ".dictation-title" }),
    ).toBeInTheDocument();
    expect(useActivity.getState().listening).toBe(true);
    expect(screen.getByRole("meter", { name: /microphone level/i })).toHaveAttribute(
      "aria-valuenow",
      "50",
    );
    expect(
      screen.getByText(/sent to gemini for transcription, then discarded/i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Stop" }));
    const box = await screen.findByLabelText(/transcript/i);
    expect(box).toHaveValue("This is a simulated transcript from the browser preview.");
    expect(useActivity.getState().listening).toBe(false);

    await user.clear(box);
    await user.type(box, "Edited words");
    await user.click(screen.getByRole("button", { name: /insert into untitled - notepad/i }));
    expect(insert).toHaveBeenCalledWith("Edited words");
    await waitFor(() => expect(screen.queryByLabelText(/transcript/i)).not.toBeInTheDocument());
  });

  it("explains microphone permission problems", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(
      recorder({
        start: vi.fn(async () => {
          throw {
            code: "permission_denied",
            message: "Wisp can't use the microphone.",
            retryable: false,
          };
        }),
      }),
    );
    const user = await openIsland();
    await user.click(await screen.findByRole("button", { name: /^dictate/i }));
    expect(await screen.findByText("Permission needed")).toBeInTheDocument();
    expect(useActivity.getState().listening).toBe(false);
  });

  it("falls back to the clipboard when insertion fails", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    vi.spyOn(h, "insertText").mockRejectedValue({
      code: "unsupported",
      message: "Couldn't paste automatically. The text is on your clipboard.",
      retryable: false,
    });
    const copy = vi.spyOn(h, "copyText").mockResolvedValue();
    const user = await openIsland();
    await user.click(await screen.findByRole("button", { name: /^dictate/i }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));
    await user.click(await screen.findByRole("button", { name: /^insert/i }));
    expect(copy).toHaveBeenCalled();
    expect(await screen.findByRole("alert")).toHaveTextContent(/on your clipboard/i);
    expect(screen.getByLabelText(/transcript/i)).toBeInTheDocument();
  });

  it("cancel stops the microphone and discards audio", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    const rec = recorder();
    vi.spyOn(h, "createRecorder").mockReturnValue(rec);
    const transcribe = vi.spyOn(h, "transcribe");
    const user = await openIsland();
    await user.click(await screen.findByRole("button", { name: /^dictate/i }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(rec.cancel).toHaveBeenCalled();
    expect(transcribe).not.toHaveBeenCalled();
    expect(useDictation.getState().phase).toBe("idle");
    expect(useActivity.getState().listening).toBe(false);
  });

  it("shortcut toggles start and stop", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(recorder());
    render(<App />);
    const mock = (window as unknown as { wispMock: { shortcut(a: string): void } }).wispMock;
    act(() => mock.shortcut("dictation"));
    await waitFor(() => expect(useDictation.getState().phase).toBe("listening"));
    act(() => mock.shortcut("dictation"));
    await waitFor(() => expect(useDictation.getState().phase).toBe("review"));
  });

  it("treats very short recordings as no speech", async () => {
    const h = await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    vi.spyOn(h, "createRecorder").mockReturnValue(
      recorder({
        stop: vi.fn(async () => ({ base64: "", mimeType: "audio/webm", durationMs: 100 })),
      }),
    );
    const transcribe = vi.spyOn(h, "transcribe");
    const user = await openIsland();
    await user.click(await screen.findByRole("button", { name: /^dictate/i }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));
    expect(
      await screen.findByText("No speech detected.", undefined, { timeout: 4000 }),
    ).toBeInTheDocument();
    expect(transcribe).not.toHaveBeenCalled();
  });
});
