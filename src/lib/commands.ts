// Command bar brain: turns what the user typed into ranked results.
// Pure functions only, so every rule here is unit-tested.

import type {
  AppUsage,
  LaunchItem,
  MediaCommand,
  OpenWindow,
  PowerAction,
  WinChange,
} from "../services/types";

export type DashboardTab =
  "today" | "controls" | "stats" | "clipboard" | "apps" | "agents" | "wallpaper" | "power";

export const DASHBOARD_TABS: { id: DashboardTab; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "controls", label: "Controls" },
  { id: "stats", label: "Stats" },
  { id: "clipboard", label: "Clipboard" },
  { id: "apps", label: "Apps" },
  { id: "agents", label: "Agents" },
  { id: "wallpaper", label: "Wallpaper" },
  { id: "power", label: "Power" },
];

/** Something the bar can do without asking the model. */
export type Command =
  | { type: "power"; action: PowerAction }
  | { type: "timer"; minutes: number }
  | { type: "stopTimer" }
  | { type: "remind"; title: string; inMinutes: number }
  | { type: "volume"; percent: number }
  | { type: "mute"; muted: boolean }
  | { type: "media"; command: MediaCommand }
  | { type: "darkMode"; dark: boolean }
  | { type: "winSetting"; change: WinChange; done: string }
  | { type: "wallpaper"; shuffle: boolean }
  | { type: "radio"; kind: "wifi" | "bluetooth"; on: boolean }
  | { type: "tab"; tab: DashboardTab }
  | { type: "settings"; tab?: string }
  | { type: "showMe"; question: string }
  | { type: "research"; topic: string }
  | { type: "replica" }
  | { type: "selectArea" }
  | { type: "dictate" }
  | { type: "tuck" }
  | { type: "clearChat" };

export interface CommandResult {
  kind: "command";
  id: string;
  title: string;
  hint: string;
  command: Command;
  /** Whether the user must confirm first. */
  confirm?: string;
}

export interface AppResult {
  kind: "app";
  id: string;
  title: string;
  hint: string;
  item: LaunchItem;
  score: number;
}

export interface AskResultItem {
  kind: "ask";
  id: "ask";
  title: string;
  hint: string;
  prompt: string;
}

/** An already-open window to switch to. */
export interface WindowResult {
  kind: "window";
  id: string;
  title: string;
  hint: string;
  window: OpenWindow;
  score: number;
}

export type BarResult = CommandResult | AppResult | WindowResult | AskResultItem;

const UNIT_MINUTES: Record<string, number> = {
  s: 1 / 60,
  sec: 1 / 60,
  secs: 1 / 60,
  second: 1 / 60,
  seconds: 1 / 60,
  m: 1,
  min: 1,
  mins: 1,
  minute: 1,
  minutes: 1,
  h: 60,
  hr: 60,
  hrs: 60,
  hour: 60,
  hours: 60,
};

/** "25", "25m", "1.5 hours", "90 sec" → minutes. */
export function parseDuration(text: string): number | null {
  const m = text.trim().match(/^(\d+(?:\.\d+)?)\s*([a-z]+)?$/i);
  if (!m) return null;
  const n = Number(m[1]);
  const unit = (m[2] ?? "m").toLowerCase();
  const mult = UNIT_MINUTES[unit];
  if (mult === undefined || !Number.isFinite(n) || n <= 0) return null;
  const minutes = n * mult;
  return minutes > 0 && minutes <= 24 * 60 ? minutes : null;
}

const TAB_WORDS: Record<string, DashboardTab> = {
  today: "today",
  calendar: "today",
  weather: "today",
  reminders: "today",
  controls: "controls",
  stats: "stats",
  system: "stats",
  performance: "stats",
  clipboard: "clipboard",
  clips: "clipboard",
  history: "clipboard",
  apps: "apps",
  games: "apps",
  launcher: "apps",
  agents: "agents",
  wallpaper: "wallpaper",
  background: "wallpaper",
  power: "power",
};

/** Exact phrases and short patterns; anything else goes to the model. */
export function parseCommand(input: string): CommandResult[] {
  const q = input.trim().replace(/\s+/g, " ");
  const l = q.toLowerCase();
  if (!l) return [];
  const out: CommandResult[] = [];
  const add = (id: string, title: string, hint: string, command: Command, confirm?: string) =>
    out.push({ kind: "command", id, title, hint, command, confirm });

  const power: Record<string, [PowerAction, string, string | undefined]> = {
    lock: ["lock", "Lock this PC", undefined],
    sleep: ["sleep", "Put the PC to sleep", "Put the PC to sleep now?"],
    restart: ["restart", "Restart in 10 seconds", "Restart Windows? Unsaved work may be lost."],
    reboot: ["restart", "Restart in 10 seconds", "Restart Windows? Unsaved work may be lost."],
    shutdown: [
      "shutdown",
      "Shut down in 10 seconds",
      "Shut down Windows? Unsaved work may be lost.",
    ],
    "shut down": [
      "shutdown",
      "Shut down in 10 seconds",
      "Shut down Windows? Unsaved work may be lost.",
    ],
    "cancel shutdown": ["cancel", "Cancel restart / shut down", undefined],
    "cancel restart": ["cancel", "Cancel restart / shut down", undefined],
  };
  const p = power[l];
  if (p) add(`power-${p[0]}`, p[1], "Power", { type: "power", action: p[0] }, p[2]);

  let m = l.match(/^(?:timer|focus|pomodoro)(?: for)?(?: (.+))?$/);
  if (m) {
    const minutes = m[1] ? parseDuration(m[1]) : 25;
    if (minutes) {
      add("timer", `Start a ${formatMinutes(minutes)} focus timer`, "Timer", {
        type: "timer",
        minutes,
      });
    }
  }
  if (l === "stop timer" || l === "cancel timer")
    add("stop-timer", "Stop the timer", "Timer", { type: "stopTimer" });

  m = q.match(/^remind me (?:to )?(.+?) in (\d+(?:\.\d+)?\s*[a-z]*)$/i);
  if (m) {
    const minutes = parseDuration(m[2]!);
    if (minutes && m[1]!.trim()) {
      const title = m[1]!.trim();
      add("remind", `Remind me “${title}” in ${formatMinutes(minutes)}`, "Reminder", {
        type: "remind",
        title: title.charAt(0).toUpperCase() + title.slice(1),
        inMinutes: minutes,
      });
    }
  }

  m = l.match(/^(?:volume|vol)(?: to)? (\d{1,3})%?$/);
  if (m) {
    const percent = Math.min(100, Number(m[1]));
    add("volume", `Set volume to ${percent}%`, "Sound", { type: "volume", percent });
  }
  if (l === "mute") add("mute", "Mute sound", "Sound", { type: "mute", muted: true });
  if (l === "unmute") add("unmute", "Unmute sound", "Sound", { type: "mute", muted: false });

  const media: Record<string, [MediaCommand, string]> = {
    play: ["playPause", "Play / pause"],
    pause: ["playPause", "Play / pause"],
    "play pause": ["playPause", "Play / pause"],
    next: ["next", "Next track"],
    skip: ["next", "Next track"],
    "next track": ["next", "Next track"],
    previous: ["previous", "Previous track"],
    prev: ["previous", "Previous track"],
    back: ["previous", "Previous track"],
  };
  const md = media[l];
  if (md) add(`media-${md[0]}`, md[1], "Media", { type: "media", command: md[0] });

  if (/^(dark mode|dark theme|go dark)$/.test(l))
    add("dark", "Switch Windows to dark mode", "Appearance", { type: "darkMode", dark: true });
  if (/^(light mode|light theme|go light)$/.test(l))
    add("light", "Switch Windows to light mode", "Appearance", { type: "darkMode", dark: false });

  for (const r of windowsCommands(l)) add(r.id, r.title, r.hint, r.command);

  m = l.match(/^(wi-?fi|bluetooth|bt) (on|off)$/);
  if (m) {
    const kind = m[1]!.startsWith("w") ? "wifi" : "bluetooth";
    const on = m[2] === "on";
    add(`radio-${kind}`, `Turn ${kind === "wifi" ? "Wi-Fi" : "Bluetooth"} ${m[2]}`, "Radios", {
      type: "radio",
      kind,
      on,
    });
  }

  const tab = TAB_WORDS[l];
  if (tab)
    add(`tab-${tab}`, `Open ${tab[0]!.toUpperCase() + tab.slice(1)}`, "Dashboard", {
      type: "tab",
      tab,
    });
  if (l === "settings" || l === "preferences")
    add("settings", "Open Wisp settings", "Wisp", { type: "settings" });
  if (/^(shortcuts?|hotkeys?|keyboard shortcuts?|change shortcuts?)$/.test(l))
    add("settings-shortcuts", "Change shortcuts", "Wisp", { type: "settings", tab: "shortcuts" });
  if (/^(hide|show|toggle) (the )?notch$/.test(l))
    add("tuck-notch", "Show or hide the notch", "Wisp", { type: "settings", tab: "general" });
  if (l === "privacy")
    add("settings-privacy", "Privacy settings", "Wisp", { type: "settings", tab: "privacy" });

  m = q.match(/^(?:show me|where(?:'s| is| are)|find on screen|point (?:to|at)) (.+)$/i);
  if (m) {
    add("show-me", `Show me ${m[1]!.replace(/\?$/, "")}`, "Looks at this screen", {
      type: "showMe",
      question: q,
    });
  }
  m = q.match(/^research (.{3,})$/i);
  if (m)
    add("research", `Research “${m[1]}”`, "Agent · web brief to Documents", {
      type: "research",
      topic: m[1]!,
    });
  if (/^(replica|recreate (this )?(page|screen|site)|clone (this )?(page|site))$/.test(l))
    add("replica", "Recreate this screen as HTML", "Agent · saves to Documents", {
      type: "replica",
    });
  if (/^(select|select area|lasso|circle|snip)$/.test(l))
    add("select", "Select an area to ask about", "Screen", { type: "selectArea" });
  if (/^(dictate|dictation|type for me)$/.test(l))
    add("dictate", "Start dictation", "Voice", { type: "dictate" });
  if (/^(tuck|hide|tuck away)$/.test(l)) add("tuck", "Tuck Wisp away", "Wisp", { type: "tuck" });
  if (/^(clear|new chat|new conversation|reset)$/.test(l))
    add("clear", "Start a new conversation", "Wisp", { type: "clearChat" });
  return out;
}

export function formatMinutes(minutes: number): string {
  if (minutes < 1) return `${Math.round(minutes * 60)} s`;
  if (minutes < 60) return `${Math.round(minutes * 10) / 10} min`;
  const h = Math.floor(minutes / 60);
  const rest = Math.round(minutes - h * 60);
  return rest ? `${h} h ${rest} min` : `${h} h`;
}

/**
 * Fuzzy score of `query` against `name` (0 = no match). Prefix beats
 * word-start beats initials beats in-order letters.
 */
export function fuzzyScore(query: string, name: string): number {
  const q = query.trim().toLowerCase();
  const n = name.toLowerCase();
  if (!q) return 0;
  if (n === q) return 1000;
  if (n.startsWith(q)) return 800 - Math.min(n.length, 100);
  const words = n.split(/[\s\-_.:]+/).filter(Boolean);
  if (words.some((w) => w.startsWith(q))) return 600 - Math.min(n.length, 100);
  const initials = words.map((w) => w[0]).join("");
  if (q.length >= 2 && initials.startsWith(q)) return 500;
  if (n.includes(q)) return 400 - n.indexOf(q);
  // In-order subsequence with a penalty for gaps.
  let i = 0;
  let gaps = 0;
  let last = -1;
  for (let j = 0; j < n.length && i < q.length; j++) {
    if (n[j] === q[i]) {
      if (last >= 0) gaps += j - last - 1;
      last = j;
      i++;
    }
  }
  if (i < q.length || q.length < 2) return 0;
  return Math.max(1, 200 - gaps * 8);
}

export function rankApps(
  query: string,
  items: LaunchItem[],
  starred: string[],
  recent: AppUsage[],
  limit = 6,
): AppResult[] {
  const recentRank = new Map(recent.map((r, i) => [r.id, r.launches * 4 + (20 - i)]));
  return items
    .map((item) => {
      const s = fuzzyScore(query, item.name);
      const boost =
        (starred.includes(item.id) ? 60 : 0) + Math.min(recentRank.get(item.id) ?? 0, 80);
      return { item, score: s ? s + boost : 0 };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score || a.item.name.localeCompare(b.item.name))
    .slice(0, limit)
    .map(({ item, score }) => ({
      kind: "app" as const,
      id: item.id,
      title: item.name,
      hint: item.kind === "game" ? "Steam game" : item.kind === "system" ? "Windows" : "App",
      item,
      score,
    }));
}

/** Looks like a question or instruction rather than an app name. */
export function looksLikeQuestion(q: string): boolean {
  const t = q.trim();
  if (/[?]$/.test(t)) return true;
  if (t.split(/\s+/).length >= 4) return true;
  return /^(what|why|how|who|when|where|which|can|could|should|would|is|are|do|does|explain|write|summari[sz]e|translate|rewrite|fix|make|tell|give|define|compare)\b/i.test(
    t,
  );
}

const SWITCH_PREFIX = /^(?:switch to|go to|jump to|show)\s+(.+)$/i;

/** Friendly app label: "chrome" → "Chrome". */
function appLabel(app: string): string {
  return app ? app[0]!.toUpperCase() + app.slice(1) : "Window";
}

/** Open windows matching `query`, by app name or window title. */
export function rankWindows(query: string, windows: OpenWindow[], limit = 4): WindowResult[] {
  const q = query.trim();
  if (!q) return [];
  return windows
    .map((w, i) => {
      const byApp = fuzzyScore(q, w.app);
      const byTitle = Math.floor(fuzzyScore(q, w.title) * 0.8);
      // Front-most windows win ties.
      const s = Math.max(byApp, byTitle);
      return { w, score: s ? s + Math.max(0, 10 - i) : 0 };
    })
    .filter((x) => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map(({ w, score }) => ({
      kind: "window" as const,
      id: `window:${w.id}`,
      title: w.title || appLabel(w.app),
      hint: `Switch to ${appLabel(w.app)}`,
      window: w,
      score,
    }));
}

/** Everything the bar shows for `query`, best first. */
export function barResults(
  query: string,
  items: LaunchItem[],
  starred: string[],
  recent: AppUsage[],
  windows: OpenWindow[] = [],
): BarResult[] {
  const q = query.trim();
  if (!q) return [];
  const switchTo = q.match(SWITCH_PREFIX)?.[1];
  if (switchTo) {
    const found = rankWindows(switchTo, windows);
    if (found.length) return found;
  }
  const commands = parseCommand(q);
  const question = looksLikeQuestion(q);
  const apps = question ? [] : rankApps(q, items, starred, recent);
  // Switching to an open window beats starting the app again.
  const open = question ? [] : rankWindows(q, windows).filter((w) => w.score >= 400);
  if (open.length) {
    const ask: AskResultItem = { kind: "ask", id: "ask", title: q, hint: "Ask Wisp", prompt: q };
    return [...commands, ...open, ...apps, ask];
  }
  const ask: AskResultItem = {
    kind: "ask",
    id: "ask",
    title: q,
    hint: "Ask Wisp",
    prompt: q,
  };
  const strongApp = apps[0] && apps[0].score >= 600;
  if (commands.length) return [...commands, ...apps, ask];
  if (strongApp) return [...apps, ask];
  return [ask, ...apps];
}

const ACCENT_NAMES: Record<string, string> = {
  blue: "#0078d4",
  teal: "#00b7c3",
  green: "#107c10",
  gold: "#ffb900",
  yellow: "#ffb900",
  orange: "#f7630c",
  red: "#e81123",
  pink: "#e3008c",
  purple: "#744da9",
};

/** "night light on", "taskbar left", "brightness 40", "next wallpaper"… */
export function windowsCommands(
  l: string,
): { id: string; title: string; hint: string; command: Command }[] {
  const out: { id: string; title: string; hint: string; command: Command }[] = [];
  const set = (id: string, title: string, change: WinChange, hint = "Windows") =>
    out.push({ id, title, hint, command: { type: "winSetting", change, done: title } });
  let m: RegExpMatchArray | null;

  if ((m = l.match(/^night ?light( on| off)?$/))) {
    const on = m[1]?.trim() !== "off";
    set("nightlight", `Turn night light ${on ? "on" : "off"}`, {
      setting: "nightLight",
      value: on,
    });
  }
  if ((m = l.match(/^(do not disturb|dnd|focus mode|quiet mode)( on| off)?$/))) {
    const on = m[2]?.trim() !== "off";
    set("dnd", `Turn Do Not Disturb ${on ? "on" : "off"}`, { setting: "doNotDisturb", value: on });
  }
  if ((m = l.match(/^taskbar (left|center|centre)$/)))
    set("taskbar-align", `Move taskbar icons to the ${m[1] === "left" ? "left" : "centre"}`, {
      setting: "taskbarAlign",
      value: m[1] === "left" ? "left" : "center",
    });
  if (/^(auto-?hide( the)? taskbar|hide( the)? taskbar)$/.test(l))
    set("taskbar-hide", "Auto-hide the taskbar", { setting: "taskbarAutoHide", value: true });
  if (/^(show( the)? taskbar|stop hiding( the)? taskbar)$/.test(l))
    set("taskbar-show", "Always show the taskbar", { setting: "taskbarAutoHide", value: false });
  if ((m = l.match(/^brightness (\d{1,3})%?$/)) && Number(m[1]) <= 100)
    set("brightness", `Set brightness to ${m[1]}%`, { setting: "brightness", value: Number(m[1]) });
  if (
    (m = l.match(
      /^(?:power mode )?(best performance|performance|balanced|efficiency|battery saver|power saver)(?: mode)?$/,
    ))
  ) {
    const value = m[1]!.includes("perf")
      ? "performance"
      : m[1] === "balanced"
        ? "balanced"
        : "efficiency";
    set("power-mode", `Power mode: ${value}`, { setting: "powerMode", value });
  }
  if ((m = l.match(/^transparency (on|off)$/)))
    set("transparency", `Turn transparency ${m[1]}`, {
      setting: "transparency",
      value: m[1] === "on",
    });
  if ((m = l.match(/^(show|hide) desktop icons$/)))
    set("desktop-icons", `${m[1] === "show" ? "Show" : "Hide"} desktop icons`, {
      setting: "desktopIcons",
      value: m[1] === "show",
    });
  if ((m = l.match(/^(mute|unmute) (the )?(mic|microphone)$/)))
    set("mic", `${m[1] === "mute" ? "Mute" : "Unmute"} the microphone`, {
      setting: "micMute",
      value: m[1] === "mute",
    });
  if ((m = l.match(/^(?:(?:switch to|use|play on|play through) )?(headphones|headset|speakers)$/)))
    set("output", `Play sound through ${m[1]}`, { setting: "outputDevice", value: m[1]! });
  if ((m = l.match(/^accent( colou?r)? (#[0-9a-f]{6}|[a-z]+)$/))) {
    const hex = m[2]!.startsWith("#") ? m[2]! : ACCENT_NAMES[m[2]!];
    if (hex)
      set("accent", `Set the accent colour to ${m[2]}`, { setting: "accentColor", value: hex });
  }
  if (/^(next|change|new) wallpaper$/.test(l))
    out.push({
      id: "wallpaper-next",
      title: "Next wallpaper",
      hint: "Wallpaper",
      command: { type: "wallpaper", shuffle: false },
    });
  if (/^(shuffle|random) wallpaper$/.test(l))
    out.push({
      id: "wallpaper-shuffle",
      title: "Random wallpaper",
      hint: "Wallpaper",
      command: { type: "wallpaper", shuffle: true },
    });
  return out;
}
