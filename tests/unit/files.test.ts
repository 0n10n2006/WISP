import { barResults, fileQuery, fileResults, pcTopic } from "../../src/lib/commands";
import { duration, pcAnswer } from "../../src/lib/pcinfo";
import { MOCK_FILES, mockFileSearch } from "../../src/services/mockHost";
import type { LaunchItem, OpenWindow, PcInfo } from "../../src/services/types";
import { ago, statusText } from "../../src/screens/settings/SearchTab";

const items: LaunchItem[] = [
  { id: "lnk:spotify", name: "Spotify", kind: "app", target: "C:\\Spotify.lnk" },
  { id: "lnk:word", name: "Word", kind: "app", target: "C:\\Word.lnk" },
];
const windows: OpenWindow[] = [{ id: 1, app: "chrome", title: "Inbox - Gmail" }];
const search = (q: string) => mockFileSearch(MOCK_FILES, fileQuery(q) ?? "");
const results = (q: string) => barResults(q, items, [], [], windows, search(q));

describe("file results in the bar", () => {
  it("knows what to look up", () => {
    expect(fileQuery("open my resume")).toBe("resume");
    expect(fileQuery("find the invoice")).toBe("invoice");
    expect(fileQuery("files budget")).toBe("budget");
    expect(fileQuery("resu")).toBe("resu");
    expect(fileQuery("r")).toBeNull();
    // Questions go to Wisp; PC questions are answered locally.
    expect(fileQuery("what is the capital of france")).toBeNull();
    expect(fileQuery("battery")).toBeNull();
  });

  it("“open …” runs the best local match: window, then app, then file", () => {
    expect(results("open my resume")[0]).toMatchObject({ kind: "file", title: "Resume.pdf" });
    expect(results("open spotify")[0]).toMatchObject({ kind: "app", title: "Spotify" });
    expect(results("open chrome")[0]).toMatchObject({ kind: "window" });
    expect(results("open holiday photos")[0]).toMatchObject({
      kind: "file",
      title: "Holiday photos",
    });
    // Nothing local: asking stays available.
    expect(results("open zzz").at(-1)).toMatchObject({ kind: "ask" });
  });

  it("“find …” shows files only", () => {
    const r = results("find invoice");
    expect(r.map((x) => x.kind)).toEqual(["file", "ask"]);
    expect(r[0]).toMatchObject({ title: "Invoice_March.pdf", hint: "PDF · Downloads/Invoices" });
  });

  it("a file whose name starts with what you typed comes first", () => {
    expect(results("resu")[0]).toMatchObject({ kind: "file", title: "Resume.pdf" });
    expect(results("spot")[0]).toMatchObject({ kind: "app" });
    expect(results("budget")[0]).toMatchObject({ kind: "file", title: "budget.xlsx" });
  });

  it("names rows by kind and folder", () => {
    const [folder] = fileResults([MOCK_FILES[7]!]);
    expect(folder).toMatchObject({ id: "file:8", hint: "Folder" });
    expect(mockFileSearch(MOCK_FILES, "inv mar")[0]?.name).toBe("Invoice_March.pdf");
  });
});

const info: PcInfo = {
  battery: { percent: 76, charging: false, pluggedIn: false, secsLeft: 3 * 3600 + 600 },
  localIp: "192.168.1.24",
  stats: {
    cpu: 23.4,
    cores: [20, 26],
    cpuName: "Ryzen 7",
    memoryTotal: 16 * 1024 ** 3,
    memoryUsed: 8 * 1024 ** 3,
    disks: [{ mount: "C:\\", totalBytes: 500 * 1024 ** 3, freeBytes: 125 * 1024 ** 3 }],
    netDown: 0,
    netUp: 0,
    uptimeSecs: 2 * 86400 + 3 * 3600,
    top: [
      { name: "chrome", count: 10, memoryBytes: 2 * 1024 ** 3, cpu: 9 },
      { name: "Code", count: 3, memoryBytes: 1024 ** 3, cpu: 4 },
    ],
  },
};

describe("quick PC answers", () => {
  it("recognises questions about this PC", () => {
    expect(pcTopic("battery")).toBe("battery");
    expect(pcTopic("how much battery do i have left?")).toBe("battery");
    expect(pcTopic("what's my ip")).toBe("ip");
    expect(pcTopic("how much disk space do i have")).toBe("disk");
    expect(pcTopic("storage")).toBe("disk");
    expect(pcTopic("what's using my memory?")).toBe("memory");
    expect(pcTopic("cpu")).toBe("cpu");
    expect(pcTopic("how long has my pc been on")).toBe("uptime");
    expect(pcTopic("battery of my phone")).toBeNull();
    expect(results("battery")[0]).toMatchObject({
      kind: "command",
      command: { type: "pcInfo", topic: "battery" },
    });
  });

  it("answers in plain words", () => {
    expect(pcAnswer("battery", info)).toBe(
      "Battery is at **76%** (about 3 hours 10 minutes left).",
    );
    expect(pcAnswer("battery", { ...info, battery: null })).toMatch(/doesn't have a battery/);
    expect(pcAnswer("disk", info)).toContain("**C:\\** 125 GB free of 500 GB (75% used)");
    expect(pcAnswer("ip", info)).toContain("**192.168.1.24**");
    expect(pcAnswer("ip", { ...info, localIp: null })).toMatch(/isn't connected/);
    expect(pcAnswer("memory", info)).toBe(
      "Using **8 GB** of 16 GB memory (50%). Most is used by chrome (2 GB), Code (1 GB).",
    );
    expect(pcAnswer("cpu", info)).toBe(
      "CPU is at **23%** (Ryzen 7). Busiest: chrome (9%), Code (4%).",
    );
    expect(pcAnswer("uptime", info)).toContain("2 days 3 hours");
    expect(duration(59)).toBe("1 minute");
  });
});

describe("search status", () => {
  it("says how big and fresh the index is", () => {
    const now = Date.now();
    expect(ago(now - 30_000, now)).toBe("just now");
    expect(ago(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(ago(now - 3 * 3600_000, now)).toBe("3 h ago");
    const base = { enabled: true, indexing: false, count: 12345, scanned: 0, folders: [] };
    expect(statusText({ ...base, updatedAt: now - 120_000 })).toMatch(
      /^12,345 items · updated 2 min ago$/,
    );
    expect(statusText({ ...base, indexing: true, scanned: 800, updatedAt: null })).toMatch(
      /Indexing… 800/,
    );
    expect(statusText({ ...base, enabled: false, updatedAt: null })).toBe("File search is off.");
  });
});
