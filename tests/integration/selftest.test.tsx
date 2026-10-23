import { act, render, waitFor } from "@testing-library/preact";
import { App } from "../../src/App";
import { setupMockHost } from "../helpers";

interface Report {
  section: string;
  steps: { name: string; ok: boolean; detail?: string; optional?: boolean }[];
}
const mock = () =>
  (
    window as unknown as {
      wispMock: { runSelfTest(s: string): void; selfTestReports: Report[]; errors: string[] };
    }
  ).wispMock;

function failures(r: Report) {
  return r.steps.filter((s) => !s.ok && !s.optional).map((s) => `${s.name}: ${s.detail}`);
}

describe("the --self-test UI scenario", () => {
  it("passes on the island against the mock host", async () => {
    await setupMockHost({ withKey: true, onboarded: true, windowKind: "island" });
    render(<App />);
    act(() => mock().runSelfTest("island"));
    await waitFor(() => expect(mock().selfTestReports).toHaveLength(1), { timeout: 40_000 });
    const report = mock().selfTestReports[0]!;
    expect(report.section).toBe("island");
    expect(failures(report)).toEqual([]);
    expect(report.steps.map((s) => s.name)).toEqual([
      "sliver-on-screen",
      "hover-peeks",
      "leave-folds-back",
      "click-opens-bar",
      "typing-shows-results",
      "dashboard-tabs",
      "escape-closes",
      "shortcut-opens-focused",
      "capture-overlay",
      "back-to-sliver",
    ]);
  }, 60_000);

  it("passes in Settings, even before setup is finished", async () => {
    await setupMockHost({ withKey: true, onboarded: false, windowKind: "settings" });
    render(<App />);
    act(() => mock().runSelfTest("settings"));
    await waitFor(() => expect(mock().selfTestReports).toHaveLength(1), { timeout: 40_000 });
    expect(failures(mock().selfTestReports[0]!)).toEqual([]);
  }, 60_000);
});
