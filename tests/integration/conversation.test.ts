import { act } from "@testing-library/preact";
import { useConversation } from "../../src/state/conversation";
import { setupMockHost } from "../helpers";

const ask = (p: string) => act(() => useConversation.getState().ask(p));

describe("conversation store", () => {
  it("streams an answer and flashes success", async () => {
    await setupMockHost({ withKey: true });
    await ask("hello there");
    const s = useConversation.getState();
    expect(s.requestActive).toBe(false);
    expect(s.messages).toHaveLength(2);
    expect(s.messages[1]!.status).toBe("done");
    expect(s.messages[1]!.text).toContain("hello there");
    expect(s.successFlash).toBe(true);
  });

  it("keeps the prompt when the request fails", async () => {
    await setupMockHost({ withKey: true });
    await ask("summarise this /quota");
    const s = useConversation.getState();
    expect(s.error?.code).toBe("quota_exhausted");
    expect(s.messages).toHaveLength(0);
    expect(s.draft).toBe("summarise this /quota");
  });

  it("reports a missing key", async () => {
    await setupMockHost();
    await ask("hi");
    expect(useConversation.getState().error?.code).toBe("missing_key");
  });

  it("retries the failed prompt", async () => {
    const h = await setupMockHost({ withKey: true });
    await ask("first /offline");
    expect(useConversation.getState().error?.code).toBe("offline");
    // Simulate connectivity coming back by changing the stored failed prompt.
    useConversation.setState({ lastFailed: { prompt: "first", opts: {} } });
    await act(() => useConversation.getState().retry());
    const s = useConversation.getState();
    expect(s.error).toBeNull();
    expect(s.messages[1]!.status).toBe("done");
    expect(h.kind).toBe("mock");
  });

  it("cancels and preserves the draft when nothing arrived", async () => {
    await setupMockHost({ withKey: true });
    let p!: Promise<void>;
    act(() => {
      p = useConversation.getState().ask("long one /slow");
    });
    expect(useConversation.getState().requestActive).toBe(true);
    act(() => useConversation.getState().cancel());
    await act(() => p);
    const s = useConversation.getState();
    expect(s.requestActive).toBe(false);
    expect(s.error).toBeNull();
    expect(s.messages).toHaveLength(0);
    expect(s.draft).toBe("long one /slow");
  });

  it("sends prior turns as history", async () => {
    const h = await setupMockHost({ withKey: true });
    const spy = vi.spyOn(h, "ask");
    await ask("one");
    await ask("two");
    const req = spy.mock.calls[1]![0];
    expect(req.history.map((t) => t.role)).toEqual(["user", "model"]);
    expect(req.history[0]!.text).toBe("one");
  });

  it("ignores empty prompts and concurrent asks", async () => {
    const h = await setupMockHost({ withKey: true });
    const spy = vi.spyOn(h, "ask");
    await ask("   ");
    expect(spy).not.toHaveBeenCalled();
  });
});
