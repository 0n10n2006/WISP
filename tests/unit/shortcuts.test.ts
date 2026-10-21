import { acceleratorFromEvent } from "../../src/components/ShortcutInput";
import { mockNormalizeShortcut } from "../../src/services/mockHost";

const ev = (
  code: string,
  mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {},
) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("acceleratorFromEvent", () => {
  it("builds accelerators in canonical order", () => {
    expect(acceleratorFromEvent(ev("KeyS", { ctrlKey: true, shiftKey: true }))).toBe(
      "Ctrl+Shift+S",
    );
    expect(acceleratorFromEvent(ev("Space", { altKey: true, metaKey: true }))).toBe(
      "Alt+Super+Space",
    );
    expect(acceleratorFromEvent(ev("Digit1", { ctrlKey: true }))).toBe("Ctrl+1");
    expect(acceleratorFromEvent(ev("F9"))).toBe("F9");
  });

  it("ignores bare modifier presses", () => {
    expect(acceleratorFromEvent(ev("ShiftLeft", { shiftKey: true }))).toBeNull();
    expect(acceleratorFromEvent(ev("ControlLeft", { ctrlKey: true }))).toBeNull();
  });
});

describe("mockNormalizeShortcut", () => {
  it("matches the Rust rules for common cases", () => {
    expect(mockNormalizeShortcut("shift+ctrl+space")).toBe("Ctrl+Shift+Space");
    expect(() => mockNormalizeShortcut("Shift+A")).toThrow();
    expect(() => mockNormalizeShortcut("Ctrl+")).toThrow();
  });
});
