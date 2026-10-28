import { calculate, evaluate, formatNumber, type Rates } from "../../src/lib/calc";
import {
  barResults,
  DEFAULT_WEB_SHORTCUTS,
  fuzzyScore,
  parseTag,
  tagForWord,
  webShortcut,
  withinOneEdit,
} from "../../src/lib/commands";
import { MOCK_PAGES, MOCK_PROCS, MOCK_TABS } from "../../src/services/mockHost";
import { DEFAULT_SETTINGS, type LaunchItem } from "../../src/services/types";

const rates: Rates = { base: "EUR", date: "2026-10-27", rates: { USD: 1.08, GBP: 0.85 } };

describe("calculator", () => {
  it("does arithmetic safely", () => {
    expect(evaluate("2+3*4")).toBe(14);
    expect(evaluate("(2+3)*4")).toBe(20);
    expect(evaluate("2^3^2")).toBe(512);
    expect(evaluate("-3^2")).toBe(-9);
    expect(evaluate("50%")).toBe(0.5);
    expect(evaluate("sqrt(16)+abs(-2)")).toBe(6);
    expect(evaluate("2pi")).toBeCloseTo(2 * Math.PI);
    expect(evaluate("1,234.5*2")).toBe(2469);
    expect(evaluate("1/0")).toBeNull();
    expect(evaluate("alert(1)")).toBeNull();
    expect(evaluate("constructor")).toBeNull();
    expect(evaluate("2+")).toBeNull();
    expect(evaluate("(2")).toBeNull();
    expect(formatNumber(1234.5)).toBe("1,234.5");
    expect(formatNumber(0.1 + 0.2)).toBe("0.3");
  });

  it("answers what the bar is given", () => {
    expect(calculate("12*7")).toMatchObject({ display: "84", copy: "84", hint: "Calculator" });
    expect(calculate("= 2+2")?.display).toBe("4");
    expect(calculate("12% of 80")?.display).toBe("9.6");
    expect(calculate("what is 3*3?")?.display).toBe("9");
    // Plain numbers and words aren't sums.
    expect(calculate("2024")).toBeNull();
    expect(calculate("spotify")).toBeNull();
    expect(calculate("pi")?.display).toBe("3.1415926536");
  });

  it("converts units", () => {
    expect(calculate("5 km in miles")?.display).toBe("3.1068559612 mi");
    expect(calculate("72f to c")?.display).toBe("22.2222222222 °C");
    expect(calculate("100 c to f")?.display).toBe("212 °F");
    expect(calculate("1 GiB in MB")?.display).toBe("1,073.741824 MB");
    expect(calculate("2 hours in minutes")?.display).toBe("120 min");
    expect(calculate("10 lb to kg")?.copy).toBe("4.5359237");
    expect(calculate("5 km in kg")).toBeNull();
  });

  it("converts currency with the day's rates", () => {
    expect(calculate("30 usd to eur", rates)).toMatchObject({
      display: "27.78 EUR",
      copy: "27.78",
      hint: "30 USD · rates of 2026-10-27",
    });
    expect(calculate("10 eur in gbp", rates)?.display).toBe("8.50 GBP");
    expect(calculate("£10 to usd", rates)?.display).toBe("12.71 USD");
    // Without rates yet, it says it's getting them.
    expect(calculate("30 usd to eur")?.hint).toMatch(/getting today's rates/);
    expect(calculate("30 usd to xyz", rates)).toBeNull();
  });
});

describe("search tags", () => {
  it("reads prefixes and words", () => {
    expect(parseTag("t: github")).toEqual({ tag: "tabs", rest: "github" });
    expect(parseTag("F:resume")).toEqual({ tag: "files", rest: "resume" });
    expect(parseTag("= 2+2")).toEqual({ tag: "calc", rest: "2+2" });
    expect(parseTag("tabs gmail")).toEqual({ tag: "tabs", rest: "gmail" });
    expect(parseTag("history rust")).toEqual({ tag: "history", rest: "rust" });
    expect(parseTag("windows update")).toEqual({ tag: null, rest: "windows update" });
    expect(parseTag("what is t: here")).toEqual({ tag: null, rest: "what is t: here" });
    expect(tagForWord("Tabs")).toBe("tabs");
    expect(tagForWord("spotify")).toBeNull();
  });

  const items: LaunchItem[] = [
    { id: "lnk:spotify", name: "Spotify", kind: "app", target: "C:\\Spotify.lnk" },
  ];
  const extras = {
    tabs: MOCK_TABS,
    history: MOCK_PAGES,
    processes: MOCK_PROCS,
    shortcuts: DEFAULT_WEB_SHORTCUTS,
  };

  it("scopes the bar to one kind", () => {
    expect(barResults("t: gmail", items, [], [], [], [], extras).map((r) => r.kind)).toEqual([
      "tab",
    ]);
    expect(barResults("t:", items, [], [], [], [], extras)).toHaveLength(3);
    expect(barResults("p: chrome", items, [], [], [], [], extras)[0]).toMatchObject({
      kind: "process",
      title: "chrome (14)",
      hint: "2.0 GB · 6% CPU",
    });
    // With nothing typed, the heaviest programs.
    expect(barResults("p:", items, [], [], [], [], extras)[0]?.title).toBe("chrome (14)");
    expect(
      barResults("h: hacker", items, [], [], [], [], { history: [MOCK_PAGES[1]!] })[0],
    ).toMatchObject({
      kind: "history",
      hint: "History · news.ycombinator.com",
    });
    expect(barResults("a: spot", items, [], [], [], [], extras)[0]?.kind).toBe("app");
    expect(barResults("= 6*7", items, [], [], [], [], extras)[0]?.title).toBe("= 42");
  });

  it("mixes the other searches into the bar", () => {
    const r = barResults("gmail", items, [], [], [], [], extras);
    expect(r[0]?.kind).toBe("ask");
    expect(r.some((x) => x.kind === "tab")).toBe(true);
    expect(barResults("12*7", items, [], [], [], [], extras)[0]?.kind).toBe("calc");
    // A weak process match isn't shown unscoped.
    expect(barResults("hro", items, [], [], [], [], extras).some((x) => x.kind === "process")).toBe(
      false,
    );
  });
});

describe("web shortcuts", () => {
  it("builds the search address", () => {
    expect(webShortcut("g cats & dogs", DEFAULT_WEB_SHORTCUTS)).toMatchObject({
      title: "Search Google for “cats & dogs”",
      url: "https://www.google.com/search?q=cats%20%26%20dogs",
    });
    expect(webShortcut("YT lofi", DEFAULT_WEB_SHORTCUTS)?.url).toContain("youtube.com");
    expect(webShortcut("g", DEFAULT_WEB_SHORTCUTS)).toBeNull();
    expect(webShortcut("zz cats", DEFAULT_WEB_SHORTCUTS)).toBeNull();
    const r = barResults("g cats", [], [], [], [], [], { shortcuts: DEFAULT_WEB_SHORTCUTS });
    expect(r[0]?.kind).toBe("web");
    expect(DEFAULT_SETTINGS.webShortcuts).toEqual(DEFAULT_WEB_SHORTCUTS);
  });
});

describe("typos", () => {
  it("finds apps with one wrong letter", () => {
    expect(withinOneEdit("spotfy", "spotify")).toBe(true);
    expect(withinOneEdit("chorme", "chrome")).toBe(true);
    expect(withinOneEdit("xyzzy", "spotify")).toBe(false);
    expect(fuzzyScore("spotfy", "Spotify")).toBeGreaterThan(0);
    expect(fuzzyScore("chorme", "Google Chrome")).toBeGreaterThan(0);
  });
});
