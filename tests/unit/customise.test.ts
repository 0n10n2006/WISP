import { visibleTabs } from "../../src/screens/dashboard/Dashboard";
import { processName } from "../../src/screens/settings/FocusTab";
import { moveTab, toggleTab } from "../../src/screens/settings/NotchTab";

describe("customisation helpers", () => {
  it("orders and filters dashboard tabs", () => {
    expect(visibleTabs(["power", "nope", "today"]).map((t) => t.id)).toEqual(["power", "today"]);
    expect(visibleTabs([]).length).toBe(8);
    expect(visibleTabs(["nope"]).length).toBe(8);
    expect(moveTab(["a", "b", "c"], "c", -1)).toEqual(["a", "c", "b"]);
    expect(moveTab(["a", "b"], "a", -1)).toEqual(["a", "b"]);
    expect(toggleTab(["a"], "a", false)).toEqual(["a"]);
    expect(toggleTab(["a", "b"], "a", false)).toEqual(["b"]);
    expect(toggleTab(["a"], "b", true)).toEqual(["a", "b"]);
  });

  it("normalises process names like the host does", () => {
    expect(processName("C:\\Program Files\\OBS\\obs64.EXE")).toBe("obs64");
    expect(processName("  Photoshop.exe ")).toBe("photoshop");
    expect(processName("")).toBe("");
  });
});
