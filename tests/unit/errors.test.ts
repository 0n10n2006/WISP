import { presentError, retryHint, toWispError } from "../../src/services/errors";

describe("errors", () => {
  it("passes WispErrors through and wraps everything else", () => {
    const e = { code: "offline", message: "x", retryable: true } as const;
    expect(toWispError(e)).toBe(e);
    expect(toWispError("boom")).toEqual({ code: "internal", message: "boom", retryable: false });
    expect(toWispError(new Error("bad")).message).toBe("bad");
    expect(toWispError({ code: "nope", message: "x" }).code).toBe("internal");
  });

  it("maps quota to a warning with a settings action", () => {
    const p = presentError({ code: "quota_exhausted", message: "", retryable: false });
    expect(p.severity).toBe("warning");
    expect(p.action).toBe("settings");
  });

  it("maps network errors to retry", () => {
    expect(presentError({ code: "offline", message: "", retryable: true }).action).toBe("retry");
    expect(presentError({ code: "timeout", message: "", retryable: true }).action).toBe("retry");
  });

  it("formats retry hints", () => {
    expect(retryHint({ code: "rate_limited", message: "", retryable: true })).toBeNull();
    expect(
      retryHint({ code: "rate_limited", message: "", retryable: true, retryAfterSecs: 20 }),
    ).toBe("Try again in about 20 s.");
    expect(
      retryHint({ code: "rate_limited", message: "", retryable: true, retryAfterSecs: 600 }),
    ).toBe("Try again in about 10 min.");
    expect(
      retryHint({
        code: "quota_exhausted",
        message: "",
        retryable: false,
        retryAfterSecs: 3 * 3600,
      }),
    ).toBe("Try again in about 3 h.");
  });
});
