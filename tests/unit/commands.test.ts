import {
  barResults,
  formatMinutes,
  fuzzyScore,
  looksLikeQuestion,
  parseCommand,
  parseDuration,
  rankApps,
} from "../../src/lib/commands";
import { chipsFor } from "../../src/lib/chips";
import type { LaunchItem } from "../../src/services/types";

const items: LaunchItem[] = [
  { id: "lnk:code", name: "Visual Studio Code", kind: "app", target: "C:\\Code.lnk" },
  { id: "lnk:spotify", name: "Spotify", kind: "app", target: "C:\\Spotify.lnk" },
  { id: "steam:570", name: "Dota 2", kind: "game", target: "steam://rungameid/570" },
  { id: "uri:ms-calculator:", name: "Calculator", kind: "system", target: "ms-calculator:" },
];

describe("parseDuration", () => {
  it("reads minutes, hours and seconds", () => {
    expect(parseDuration("25")).toBe(25);
    expect(parseDuration("25m")).toBe(25);
    expect(parseDuration("1.5 hours")).toBe(90);
    expect(parseDuration("90 sec")).toBe(1.5);
    expect(parseDuration("0")).toBeNull();
    expect(parseDuration("3 days")).toBeNull();
    expect(parseDuration("25 hours")).toBeNull();
    expect(formatMinutes(90)).toBe("1 h 30 min");
    expect(formatMinutes(0.5)).toBe("30 s");
  });
});

describe("parseCommand", () => {
  const one = (q: string) => parseCommand(q)[0]?.command;
  it("understands quick commands", () => {
    expect(one("timer")).toEqual({ type: "timer", minutes: 25 });
    expect(one("focus 50")).toEqual({ type: "timer", minutes: 50 });
    expect(one("volume 30")).toEqual({ type: "volume", percent: 30 });
    expect(one("vol to 250")).toEqual({ type: "volume", percent: 100 });
    expect(one("mute")).toEqual({ type: "mute", muted: true });
    expect(one("next")).toEqual({ type: "media", command: "next" });
    expect(one("Dark mode")).toEqual({ type: "darkMode", dark: true });
    expect(one("wifi off")).toEqual({ type: "radio", kind: "wifi", on: false });
    expect(one("bluetooth on")).toEqual({ type: "radio", kind: "bluetooth", on: true });
    expect(one("lock")).toEqual({ type: "power", action: "lock" });
    expect(one("stats")).toEqual({ type: "tab", tab: "stats" });
    expect(one("tuck")).toEqual({ type: "tuck" });
  });

  it("parses reminders and agents", () => {
    expect(one("remind me to stretch in 20 min")).toEqual({
      type: "remind",
      title: "Stretch",
      inMinutes: 20,
    });
    expect(one("research solid-state batteries")).toEqual({
      type: "research",
      topic: "solid-state batteries",
    });
    expect(one("where is the save button?")).toMatchObject({ type: "showMe" });
  });

  it("asks before consequential power actions", () => {
    expect(parseCommand("shutdown")[0]!.confirm).toMatch(/unsaved work/i);
    expect(parseCommand("restart")[0]!.confirm).toBeTruthy();
    expect(parseCommand("lock")[0]!.confirm).toBeUndefined();
  });

  it("never treats arbitrary text as a command", () => {
    expect(parseCommand("rm -rf /")).toEqual([]);
    expect(parseCommand("powershell Remove-Item")).toEqual([]);
    expect(parseCommand("")).toEqual([]);
  });
});

describe("fuzzy app search", () => {
  it("ranks prefix > word start > initials > subsequence", () => {
    expect(fuzzyScore("spo", "Spotify")).toBeGreaterThan(fuzzyScore("spo", "Display options"));
    expect(fuzzyScore("vsc", "Visual Studio Code")).toBeGreaterThan(0);
    expect(fuzzyScore("code", "Visual Studio Code")).toBeGreaterThan(
      fuzzyScore("vsc", "Visual Studio Code"),
    );
    expect(fuzzyScore("zzz", "Spotify")).toBe(0);
  });

  it("boosts starred and recent apps", () => {
    const plain = rankApps("c", items, [], []);
    const starred = rankApps("c", items, ["uri:ms-calculator:"], []);
    expect(starred[0]!.id).toBe("uri:ms-calculator:");
    expect(plain.length).toBeGreaterThan(0);
  });
});

describe("barResults", () => {
  it("puts strong app matches first and questions first otherwise", () => {
    expect(barResults("spotify", items, [], [])[0]).toMatchObject({
      kind: "app",
      id: "lnk:spotify",
    });
    expect(barResults("why is the sky blue?", items, [], [])[0]).toMatchObject({ kind: "ask" });
    expect(barResults("timer 10", items, [], [])[0]).toMatchObject({ kind: "command" });
    // Asking is always available.
    expect(barResults("spotify", items, [], []).some((r) => r.kind === "ask")).toBe(true);
    expect(looksLikeQuestion("how do I")).toBe(true);
    expect(looksLikeQuestion("dota")).toBe(false);
  });
});

describe("app-aware chips", () => {
  it("suggests by app, with a general fallback", () => {
    expect(chipsFor("code").map((c) => c.id)).toContain("explain-code");
    expect(chipsFor("chrome").map((c) => c.id)).toContain("summarise-page");
    expect(chipsFor("winword").map((c) => c.id)).toContain("rewrite-formal");
    expect(chipsFor(null).length).toBeGreaterThan(0);
  });
});

describe("open windows in the bar", () => {
  const windows = [
    { id: 11, app: "chrome", title: "Quarterly report - Google Docs" },
    { id: 12, app: "code", title: "notch.css - WISP - Visual Studio Code" },
    { id: 13, app: "spotify", title: "Spotify Premium" },
  ];

  it("offers switching before launching again", () => {
    const r = barResults("spotify", items, [], [], windows);
    expect(r[0]).toMatchObject({ kind: "window", hint: "Switch to Spotify" });
    expect(r.some((x) => x.kind === "app" && x.title === "Spotify")).toBe(true);
  });

  it("understands “switch to …” by app or title", () => {
    expect(barResults("switch to chrome", items, [], [], windows)).toEqual([
      expect.objectContaining({ kind: "window", window: windows[0] }),
    ]);
    expect(barResults("go to quarterly", items, [], [], windows)[0]).toMatchObject({
      kind: "window",
      window: { id: 11 },
    });
  });

  it("doesn't hijack questions", () => {
    expect(barResults("why is chrome slow?", items, [], [], windows)[0]).toMatchObject({
      kind: "ask",
    });
  });
});
