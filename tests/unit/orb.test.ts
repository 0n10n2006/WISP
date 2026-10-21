import { deriveOrbState, ORB_LABELS, type OrbInputs } from "../../src/state/orb";

const base: OrbInputs = {
  listening: false,
  requestActive: false,
  errorSeverity: null,
  successFlash: false,
};

describe("deriveOrbState", () => {
  it("is idle by default", () => {
    expect(deriveOrbState(base)).toBe("idle");
  });

  it("never hides capture behind other states", () => {
    expect(
      deriveOrbState({ ...base, listening: true, requestActive: true, errorSeverity: "error" }),
    ).toBe("listening");
  });

  it("orders thinking > error > warning > success", () => {
    expect(deriveOrbState({ ...base, requestActive: true, errorSeverity: "error" })).toBe(
      "thinking",
    );
    expect(deriveOrbState({ ...base, errorSeverity: "error", successFlash: true })).toBe("error");
    expect(deriveOrbState({ ...base, errorSeverity: "warning", successFlash: true })).toBe(
      "warning",
    );
    expect(deriveOrbState({ ...base, successFlash: true })).toBe("success");
  });

  it("has a text label for every state", () => {
    for (const label of Object.values(ORB_LABELS)) expect(label.length).toBeGreaterThan(0);
    expect(Object.keys(ORB_LABELS)).toHaveLength(6);
  });
});
