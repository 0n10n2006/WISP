// Browser stand-in for the desktop host, used by `npm run dev` and tests.
// Gemini is simulated. Type "/quota", "/offline", "/rate", "/slow" or "/error"
// into a prompt to exercise error states.

import type { WispError } from "./errors";
import type { AskHandle, Host, Unsubscribe, WindowKind } from "./host";
import {
  DEFAULT_SETTINGS,
  DEFAULT_SHORTCUTS,
  type AskResult,
  type ConnectionReport,
  type DeepPartial,
  type IslandLayout,
  type ModelInfo,
  type Settings,
  type SettingsSnapshot,
  type ShortcutAction,
} from "./types";

const STORAGE_KEY = "wisp.mock.settings";
const KEY_FLAG = "wisp.mock.hasKey";

export const MOCK_MODELS: ModelInfo[] = [
  {
    id: "gemini-2.5-flash",
    displayName: "Gemini 2.5 Flash",
    description: "Fast, capable, free-tier eligible",
    inputTokenLimit: 1048576,
  },
  {
    id: "gemini-2.5-flash-lite",
    displayName: "Gemini 2.5 Flash-Lite",
    description: "Lowest latency",
    inputTokenLimit: 1048576,
  },
  {
    id: "gemini-2.5-pro",
    displayName: "Gemini 2.5 Pro",
    description: "Most capable; tighter free limits",
    inputTokenLimit: 1048576,
  },
];

function safeGet(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeSet(key: string, value: string | null): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: keep in memory only */
  }
}

function deepMerge<T>(base: T, patch: unknown): T {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) return patch as T;
  const out: Record<string, unknown> = { ...(base as Record<string, unknown>) };
  for (const [k, v] of Object.entries(patch)) {
    const b = out[k];
    out[k] =
      typeof v === "object" &&
      v !== null &&
      !Array.isArray(v) &&
      typeof b === "object" &&
      b !== null
        ? deepMerge(b, v)
        : v;
  }
  return out as T;
}

const MODIFIERS: Record<string, string> = {
  ctrl: "Ctrl",
  control: "Ctrl",
  cmdorctrl: "Ctrl",
  alt: "Alt",
  option: "Alt",
  shift: "Shift",
  super: "Super",
  win: "Super",
  meta: "Super",
};
const ORDER = ["Ctrl", "Alt", "Shift", "Super"];

export function mockNormalizeShortcut(accel: string): string {
  const parts = accel.split("+").map((p) => p.trim());
  if (parts.some((p) => !p)) throw err("invalid", "Shortcut is incomplete");
  const key = parts.pop()!;
  const mods = new Set<string>();
  for (const m of parts) {
    const name = MODIFIERS[m.toLowerCase()];
    if (!name) throw err("invalid", `Unknown modifier "${m}"`);
    mods.add(name);
  }
  let k = key.length === 1 ? key.toUpperCase() : key[0]!.toUpperCase() + key.slice(1);
  if (k.toLowerCase() === "space") k = "Space";
  const isF = /^F([1-9]|1[0-9]|2[0-4])$/.test(k);
  if (!isF && !(mods.has("Ctrl") || mods.has("Alt") || mods.has("Super"))) {
    throw err("invalid", "Use Ctrl, Alt or Win so the shortcut doesn't interfere with typing");
  }
  return [...ORDER.filter((m) => mods.has(m)), k].join("+");
}

function err(code: WispError["code"], message: string, extra: Partial<WispError> = {}): WispError {
  return { code, message, retryable: false, ...extra };
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(resolve, ms);
    signal.addEventListener("abort", () => {
      clearTimeout(t);
      reject(err("cancelled", "Cancelled"));
    });
  });
}

function mockAnswer(prompt: string): string {
  const p = prompt.trim();
  return [
    `Here's a quick take on **"${p.length > 60 ? p.slice(0, 60) + "…" : p}"**.`,
    "",
    "This is a simulated answer from the browser preview — no request left your device. In the desktop app, Wisp sends your prompt to Gemini with your own API key.",
    "",
    "- Responses stream in as they arrive",
    "- You can **stop** at any time",
    "- Copy the answer with one click",
    "",
    "```ts",
    'const wisp = "quietly capable";',
    "```",
  ].join("\n");
}

export interface MockHostOptions {
  /** Delay between streamed words, ms. */
  tokenDelay?: number;
  windowKind?: WindowKind;
  persist?: boolean;
}

export function createMockHost(opts: MockHostOptions = {}): Host {
  const params = new URLSearchParams(typeof window !== "undefined" ? window.location.search : "");
  const kind: WindowKind =
    opts.windowKind ?? ((params.get("window") as WindowKind | null) || "island");
  const persist = opts.persist ?? true;
  const tokenDelay = opts.tokenDelay ?? 25;

  let settings: Settings = DEFAULT_SETTINGS;
  if (persist) {
    try {
      const raw = safeGet(STORAGE_KEY);
      if (raw) settings = deepMerge(DEFAULT_SETTINGS, JSON.parse(raw));
    } catch {
      settings = DEFAULT_SETTINGS;
    }
  }
  let hasKey = persist && safeGet(KEY_FLAG) === "1";
  const listeners = {
    settings: new Set<(s: SettingsSnapshot) => void>(),
    layout: new Set<(l: IslandLayout) => void>(),
    tab: new Set<(t: string) => void>(),
    focus: new Set<() => void>(),
    shortcut: new Set<(a: ShortcutAction) => void>(),
  };

  const snapshot = (): SettingsSnapshot => ({
    settings,
    hasApiKey: hasKey,
    shortcutIssues: [],
    recoveredFrom: null,
    version: "0.1.0-preview",
  });
  const emit = () => {
    const s = snapshot();
    listeners.settings.forEach((cb) => cb(s));
    return s;
  };
  const save = () => {
    if (persist) safeSet(STORAGE_KEY, JSON.stringify(settings));
    return emit();
  };
  const on = <T>(set: Set<T>, cb: T): Unsubscribe => {
    set.add(cb);
    return () => set.delete(cb);
  };

  const validate = (s: Settings): Settings => {
    const shortcuts = { ...s.globalShortcuts };
    const seen = new Map<string, string>();
    for (const [name, value] of Object.entries(shortcuts) as [ShortcutAction, string][]) {
      if (!value.trim()) {
        shortcuts[name] = "";
        continue;
      }
      const n = mockNormalizeShortcut(value);
      const other = seen.get(n);
      if (other) throw err("invalid", `${n} is assigned to both ${other} and ${name}`);
      seen.set(n, name);
      shortcuts[name] = n;
    }
    if (!["system", "dark", "light"].includes(s.theme)) throw err("invalid", "Invalid theme");
    if (s.historyRetentionDays < 0 || s.historyRetentionDays > 365)
      throw err("invalid", "History retention must be 0–365 days");
    return { ...s, globalShortcuts: shortcuts, telemetryOptIn: false };
  };

  // Expose a hook so the preview page can simulate global shortcuts.
  if (typeof window !== "undefined") {
    (window as unknown as { wispMock?: unknown }).wispMock = {
      shortcut: (a: ShortcutAction) => listeners.shortcut.forEach((cb) => cb(a)),
      focus: () => listeners.focus.forEach((cb) => cb()),
    };
  }

  return {
    kind: "mock",
    windowKind: () => kind,
    initialTab: () => params.get("tab"),

    getSettings: async () => snapshot(),
    updateSettings: async (patch: DeepPartial<Settings>) => {
      settings = validate(deepMerge(settings, patch));
      return save();
    },
    resetShortcuts: async () => {
      settings = { ...settings, globalShortcuts: DEFAULT_SHORTCUTS };
      return save();
    },
    resetSettings: async () => {
      settings = { ...DEFAULT_SETTINGS, onboardingComplete: settings.onboardingComplete };
      return save();
    },
    validateShortcut: async (a) => mockNormalizeShortcut(a),
    onSettings: (cb) => on(listeners.settings, cb),

    setApiKey: async (key) => {
      if (key.trim().length < 20) throw err("invalid", "That doesn't look like a Gemini API key.");
      hasKey = true;
      if (persist) safeSet(KEY_FLAG, "1");
      return emit();
    },
    removeApiKey: async () => {
      hasKey = false;
      if (persist) safeSet(KEY_FLAG, null);
      return emit();
    },
    testConnection: async (key, model): Promise<ConnectionReport> => {
      await new Promise((r) => setTimeout(r, 300));
      const k = key ?? (hasKey ? "stored-key-xxxxxxxxxxxxxxxx" : "");
      if (!k) throw err("missing_key", "Add your Gemini API key first.");
      if (k.toLowerCase().startsWith("bad"))
        throw err("invalid_key", "Your Gemini API key was rejected.");
      const m = model ?? settings.selectedModel;
      return {
        ok: true,
        modelAvailable: MOCK_MODELS.some((x) => x.id === m),
        models: MOCK_MODELS,
        latencyMs: 300,
      };
    },
    listModels: async () => MOCK_MODELS,

    ask(req, onDelta): AskHandle {
      const ctl = new AbortController();
      const result = (async (): Promise<AskResult> => {
        const started = Date.now();
        if (!hasKey)
          throw err("missing_key", "Add your Gemini API key in Settings → Model to ask Wisp.");
        const p = req.prompt.toLowerCase();
        await sleep(p.includes("/slow") ? 2500 : 350, ctl.signal);
        if (p.includes("/quota"))
          throw err(
            "quota_exhausted",
            "You've used today's Gemini quota for this model. Try again later, switch to another model, or set up billing in Google AI Studio.",
            { retryAfterSecs: 3 * 3600 },
          );
        if (p.includes("/rate"))
          throw err(
            "rate_limited",
            "Gemini is rate-limiting requests right now. Wait a moment and try again.",
            {
              retryAfterSecs: 20,
              retryable: true,
            },
          );
        if (p.includes("/offline"))
          throw err(
            "offline",
            "Couldn't reach Gemini. Check your internet connection; local features still work.",
            {
              retryable: true,
            },
          );
        if (p.includes("/error")) throw err("internal", "Unexpected failure (ref 1a2b3c4d)");
        const text = mockAnswer(req.prompt);
        const words = text.split(/(\s+)/);
        let streamed = "";
        for (const w of words) {
          await sleep(tokenDelay, ctl.signal);
          streamed += w;
          onDelta(w);
        }
        return {
          text: streamed,
          model: settings.selectedModel,
          latencyMs: Date.now() - started,
          usage: null,
          finishReason: "STOP",
        };
      })();
      return { result, cancel: () => ctl.abort() };
    },

    setIslandExpanded: async (expanded) => {
      const l = { expanded, alignRight: false, alignBottom: false };
      listeners.layout.forEach((cb) => cb(l));
      return l;
    },
    onIslandLayout: (cb) => on(listeners.layout, cb),
    hideIsland: async () => {
      listeners.layout.forEach((cb) =>
        cb({ expanded: false, alignRight: false, alignBottom: false }),
      );
    },
    startDragging: async () => {},
    openSettings: async (tab) => {
      if (kind === "settings") {
        if (tab) listeners.tab.forEach((cb) => cb(tab));
        return;
      }
      window.open(`?window=settings${tab ? `&tab=${tab}` : ""}`, "wisp-settings");
    },
    onSettingsTab: (cb) => on(listeners.tab, cb),
    onFocusPrompt: (cb) => on(listeners.focus, cb),
    onShortcut: (cb) => on(listeners.shortcut, cb),
    quit: async () => {},

    openExternal: async (url) => {
      window.open(url, "_blank", "noopener");
    },
    copyText: async (text) => {
      await navigator.clipboard?.writeText(text);
    },
  };
}
