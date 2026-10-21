import { resolveMotion, resolveTheme } from "../../src/hooks/useTheme";

describe("theme resolution", () => {
  it("follows the system when set to System", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("resolves motion", () => {
    expect(resolveMotion("system", true)).toBe("reduced");
    expect(resolveMotion("system", false)).toBe("full");
    expect(resolveMotion("full", true)).toBe("full");
  });
});
