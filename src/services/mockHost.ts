// Browser stand-in for the desktop host, used by `npm run dev` and tests.
// Gemini is simulated. Type "/quota", "/offline", "/rate", "/slow" or "/error"
// into a prompt to exercise error states.

import pkg from "../../package.json";
import type { WispError } from "./errors";
import type { AskHandle, Host, SelfTestStep, Unsubscribe, WindowKind } from "./host";
import {
  DEFAULT_SETTINGS,
  type ActionAudit,
  type CaptureReady,
  type MemoryEntry,
  type NewTask,
  type Task,
  type RequestLog,
  type StorageStats,
  DEFAULT_SHORTCUTS,
  type AskResult,
  type ConnectionReport,
  type DeepPartial,
  type IslandLayout,
  type ModelInfo,
  type Settings,
  type SettingsSnapshot,
  type ShortcutAction,
  type AgentJob,
  type ClipEntry,
  type ControlsState,
  type HoldEvent,
  type LaunchItem,
  type AppUsage,
  type NotchMode,
  type FreeModel,
  type ModelUsage,
  type WinSettingsState,
  type Heard,
  type VoiceTarget,
} from "./types";

const STORAGE_KEY = "wisp.mock.settings";
const KEY_FLAG = "wisp.mock.hasKey";

export const MOCK_FREE_MODELS: FreeModel[] = [
  { id: "meta-llama/llama-3.3-70b-instruct:free", name: "Llama 3.3 70B", contextLength: 131072 },
  { id: "mistralai/mistral-7b-instruct:free", name: "Mistral 7B", contextLength: 32768 },
];

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

/** A fake "screenshot" for the browser preview. */
export function sampleCaptureSvg(w: number, h: number): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 160 90">
<rect width="160" height="90" fill="#e9eef5"/><rect x="0" y="0" width="160" height="8" fill="#1a263b"/>
<rect x="8" y="16" width="70" height="6" rx="2" fill="#44506a"/><rect x="8" y="28" width="140" height="3" rx="1.5" fill="#9aa6ba"/>
<rect x="8" y="35" width="128" height="3" rx="1.5" fill="#9aa6ba"/><rect x="8" y="42" width="136" height="3" rx="1.5" fill="#9aa6ba"/>
<rect x="8" y="54" width="60" height="28" rx="3" fill="#78e4f2"/><rect x="76" y="54" width="76" height="28" rx="3" fill="#b6a0ff"/>
</svg>`;
}

export interface MockHostOptions {
  /** Delay between streamed words, ms. */
  tokenDelay?: number;
  windowKind?: WindowKind;
  persist?: boolean;
  /** What Windows voice typing "types" into Wisp's bar when asked by voice. */
  voice?: string;
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
  let hasOpenrouter = false;
  const scheduledListeners = new Set<
    (r: { description: string; outcome: string; message: string }) => void
  >();
  let win: WinSettingsState = {
    supported: true,
    appsDark: true,
    systemDark: true,
    transparency: true,
    accentColor: "#0078d4",
    accentFromWallpaper: false,
    accentOnTaskbar: false,
    taskbarAlign: "center",
    taskbarAutoHide: false,
    desktopIcons: true,
    startRecent: true,
    brightness: 70,
    nightLight: false,
    doNotDisturb: false,
    powerMode: "balanced",
    outputs: [
      { id: "spk", name: "Speakers (Realtek Audio)", default: true },
      { id: "hp", name: "Headphones (WH-1000XM5)", default: false },
    ],
    micMuted: false,
  };
  const folderImages = (folders: string[]) =>
    folders.flatMap((f) => [1, 2, 3, 4, 5, 6].map((n) => `${f}\\wallpaper-${n}.jpg`));
  let currentWall: string | null = null;
  const modelUse: ModelUsage[] = [];
  const listeners = {
    settings: new Set<(s: SettingsSnapshot) => void>(),
    layout: new Set<(l: IslandLayout) => void>(),
    tab: new Set<(t: string) => void>(),
    focus: new Set<() => void>(),
    shortcut: new Set<(a: ShortcutAction) => void>(),
    data: new Set<(scope: string) => void>(),
    reminder: new Set<(t: Task) => void>(),
    captureReady: new Set<(c: CaptureReady) => void>(),
    captureError: new Set<(e: WispError) => void>(),
    tucked: new Set<(t: boolean) => void>(),
    blur: new Set<() => void>(),
    hold: new Set<(e: HoldEvent) => void>(),
    speech: new Set<(h: Heard) => void>(),
    speechDone: new Set<(h: Heard) => void>(),
    selfTest: new Set<(section: string) => void>(),
  };
  let speechTimer: ReturnType<typeof setInterval> | null = null;
  let speechTarget: VoiceTarget = "app";
  let speechHeard = "";
  const endSpeech = () => {
    if (!speechTimer) return;
    clearInterval(speechTimer);
    speechTimer = null;
    const h = { target: speechTarget, text: speechHeard };
    speechHeard = "";
    listeners.speechDone.forEach((cb) => cb(h));
  };
  const errors: string[] = [];
  const selfTestReports: { section: string; steps: SelfTestStep[] }[] = [];
  let layout: IslandLayout = { mode: "sliver", expanded: false, height: 0 };
  const metrics = { modeCalls: 0, opens: 0 };
  const setLayout = (mode: NotchMode, height?: number) => {
    metrics.modeCalls += 1;
    if (mode === "open" && layout.mode !== "open") metrics.opens += 1;
    layout = {
      mode,
      expanded: mode === "open",
      height: mode === "open" ? Math.max(96, height ?? layout.height) : layout.height,
    };
    listeners.layout.forEach((cb) => cb(layout));
    return layout;
  };
  const now = Date.now();
  let clips: ClipEntry[] = [
    {
      id: "clip-1",
      kind: "link",
      text: "https://github.com/0n10n2006/WISP",
      createdAt: new Date(now - 60_000).toISOString(),
      pinned: true,
    },
    {
      id: "clip-2",
      kind: "text",
      text: "Meeting moved to Thursday 3pm — bring the Q3 numbers.",
      createdAt: new Date(now - 300_000).toISOString(),
      pinned: false,
    },
    {
      id: "clip-3",
      kind: "text",
      text: "const answer = items.filter((i) => i.done).length;",
      createdAt: new Date(now - 900_000).toISOString(),
      pinned: false,
    },
  ];
  const clipsChanged = () => listeners.data.forEach((cb) => cb("clipboard"));
  const catalogue: LaunchItem[] = [
    { id: "lnk:spotify", name: "Spotify", kind: "app", target: "C:\\Spotify.lnk" },
    { id: "lnk:code", name: "Visual Studio Code", kind: "app", target: "C:\\Code.lnk" },
    { id: "lnk:word", name: "Word", kind: "app", target: "C:\\Word.lnk" },
    { id: "lnk:chrome", name: "Google Chrome", kind: "app", target: "C:\\Chrome.lnk" },
    { id: "lnk:discord", name: "Discord", kind: "app", target: "C:\\Discord.lnk" },
    { id: "steam:570", name: "Dota 2", kind: "game", target: "steam://rungameid/570" },
    { id: "steam:620", name: "Portal 2", kind: "game", target: "steam://rungameid/620" },
    { id: "uri:ms-calculator:", name: "Calculator", kind: "system", target: "ms-calculator:" },
    { id: "uri:ms-settings:", name: "Settings", kind: "system", target: "ms-settings:" },
    {
      id: "uri:ms-settings:display",
      name: "Display settings",
      kind: "system",
      target: "ms-settings:display",
    },
  ];
  let usage: AppUsage[] = [];
  let controls: ControlsState = {
    supported: true,
    volume: 42,
    muted: false,
    nowPlaying: { title: "Weightless", artist: "Marconi Union", playing: true, app: "Spotify" },
    radios: [
      { kind: "wifi", name: "Wi-Fi", on: true },
      { kind: "bluetooth", name: "Bluetooth", on: false },
    ],
    darkMode: true,
  };
  let jobs: AgentJob[] = [];
  const jobsChanged = () => listeners.data.forEach((cb) => cb("jobs"));
  let requests: RequestLog[] = [];
  let tasks: Task[] = [];
  let memories: MemoryEntry[] = [];
  const pendingActions = new Map<string, (approved: boolean) => void>();
  const makeTask = (t: NewTask): Task => {
    const title = t.title.trim();
    if (!title) throw err("invalid", "Reminder needs a title");
    if (t.dueAt && new Date(t.dueAt).getTime() < Date.now() - 60_000)
      throw err("invalid", "Reminder time is in the past");
    return {
      id: `task-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      title,
      notes: t.notes?.trim() || null,
      dueAt: t.dueAt ?? null,
      createdAt: new Date().toISOString(),
      completedAt: null,
      notificationEnabled: t.notificationEnabled ?? true,
      notifiedAt: null,
      repeat: t.repeat ?? "none",
    };
  };
  const taskChanged = () => listeners.data.forEach((cb) => cb("tasks"));
  const findTask = (id: string) => {
    const t = tasks.find((x) => x.id === id);
    if (!t) throw err("invalid", "That reminder no longer exists");
    return t;
  };
  const replaceTask = (t: Task) => {
    tasks = tasks.map((x) => (x.id === t.id ? t : x));
    taskChanged();
    return t;
  };
  let actions: ActionAudit[] = [];

  const logRequest = (
    prompt: string,
    contextKind: string,
    latencyMs: number,
    errorCode: string | null,
    response: string | null,
  ) => {
    if (settings.historyRetentionDays === 0) return;
    const entry: RequestLog = {
      id: `log-${Date.now()}-${requests.length}`,
      timestamp: new Date().toISOString(),
      requestType: "ask",
      model: settings.selectedModel,
      status: errorCode === "cancelled" ? "cancelled" : errorCode ? "error" : "ok",
      latencyMs,
      errorCode: errorCode === "cancelled" ? "cancelled" : errorCode,
      contextKind,
      totalTokens: null,
      prompt: settings.contentHistory ? prompt : null,
      response: settings.contentHistory ? response : null,
    };
    requests = [entry, ...requests].slice(0, 500);
  };
  const stats = (): StorageStats => ({
    requests: requests.length,
    actions: actions.length,
    tasks: tasks.length,
    openTasks: tasks.filter((t) => !t.completedAt).length,
    memories: memories.length,
    clips: clips.length,
    schemaVersion: 2,
  });

  const snapshot = (): SettingsSnapshot => ({
    settings,
    hasApiKey: hasKey,
    hasOpenrouterKey: hasOpenrouter,
    shortcutIssues: [],
    recoveredFrom: null,
    warnings: [],
    version: `${pkg.version}-preview`,
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
    const names = new Set<string>();
    const allowedApps = s.allowedApps.map((a) => {
      const name = a.name.trim().toLowerCase();
      const target = a.target.trim();
      const file = target.toLowerCase().split(/[\\/]/).pop() ?? "";
      if (!name || !target) throw err("invalid", "Allowed apps need a name and a target");
      if (names.has(name)) throw err("invalid", `App "${name}" is listed twice`);
      if (/["'&|<>^%;`$]/.test(target) || !/\.(exe|lnk)$/i.test(target))
        throw err("invalid", "App target must be an .exe or shortcut (.lnk)");
      if (
        [
          "cmd.exe",
          "powershell.exe",
          "pwsh.exe",
          "wscript.exe",
          "cscript.exe",
          "mshta.exe",
        ].includes(file)
      )
        throw err("invalid", "Shells and script hosts cannot be allowlisted");
      names.add(name);
      return { name, target };
    });
    return { ...s, allowedApps, globalShortcuts: shortcuts, telemetryOptIn: false };
  };

  // Expose a hook so the preview page can simulate global shortcuts.
  if (typeof window !== "undefined") {
    (window as unknown as { wispMock?: unknown }).wispMock = {
      shortcut: (a: ShortcutAction) => listeners.shortcut.forEach((cb) => cb(a)),
      reminder: () =>
        listeners.reminder.forEach((cb) =>
          cb({
            id: "preview-reminder",
            title: "Stand up and stretch",
            notes: null,
            dueAt: new Date().toISOString(),
            createdAt: new Date().toISOString(),
            completedAt: null,
            notificationEnabled: true,
            notifiedAt: new Date().toISOString(),
            repeat: "none",
          }),
        ),
      focus: () => {
        setLayout("open");
        listeners.focus.forEach((cb) => cb());
      },
      metrics,
      errors,
      selfTestReports,
      runSelfTest: (section = "island") => listeners.selfTest.forEach((cb) => cb(section)),
      hold: (e: HoldEvent) => {
        if (e.phase === "start" && layout.mode !== "open") setLayout("peek");
        listeners.hold.forEach((cb) => cb(e));
      },
      blur: () => listeners.blur.forEach((cb) => cb()),
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
    testOpenrouterKey: async (key) => {
      await new Promise((r) => setTimeout(r, 200));
      const k = key ?? (hasOpenrouter ? "sk-or-v1-stored-xxxxxxxxxxxx" : "");
      if (!k) throw err("missing_key", "No OpenRouter key is saved.");
      if (!k.startsWith("sk-or-"))
        throw err("invalid", "That doesn't look like an OpenRouter key (it starts with sk-or-).");
      return MOCK_FREE_MODELS;
    },
    setOpenrouterKey: async (key) => {
      if (!key.trim().startsWith("sk-or-"))
        throw err("invalid", "That doesn't look like an OpenRouter key (it starts with sk-or-).");
      hasOpenrouter = true;
      return emit();
    },
    removeOpenrouterKey: async () => {
      hasOpenrouter = false;
      return emit();
    },
    modelUsage: async () => modelUse,
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

    ask(req, onDelta, onAction): AskHandle {
      const ctl = new AbortController();
      const started = Date.now();
      const run = (async (): Promise<AskResult> => {
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
        const audit = (
          actionType: string,
          summary: string,
          outcome: ActionAudit["outcome"],
          confirmed: boolean,
        ) => {
          actions = [
            {
              id: `act-${Date.now()}-${actions.length}`,
              timestamp: new Date().toISOString(),
              actionType,
              summary,
              outcome,
              userConfirmed: confirmed,
              detail: null,
            },
            ...actions,
          ];
        };
        // Like the real host: ask only when it's risky (or always, if set).
        const untrusted =
          req.contextKind === "screen" ||
          req.contextKind === "clipboard" ||
          req.attachments.length > 0 ||
          !!req.web ||
          !!req.outsideContent;
        const propose = async (
          kind: string,
          description: string,
          delaySecs = 0,
        ): Promise<boolean> => {
          const ask =
            settings.confirmActions === "always" ||
            untrusted ||
            kind === "open_url" ||
            delaySecs > 3600;
          if (!ask) return true;
          const id = `confirm-${Date.now()}`;
          onAction?.({ type: "actionProposed", id, kind, description });
          const approved = await new Promise<boolean>((resolve) => {
            pendingActions.set(id, resolve);
            ctl.signal.addEventListener("abort", () => resolve(false));
          });
          pendingActions.delete(id);
          return approved;
        };
        if (p.includes("/open")) {
          const description = "Open https://example.com in your browser";
          const ok = await propose("open_url", description);
          audit("open_url", "host=example.com", ok ? "done" : "declined", ok);
          onAction?.({
            type: "actionResult",
            kind: "open_url",
            description,
            outcome: ok ? "done" : "declined",
            message: ok ? "Opened the link in the default browser." : "You chose not to do this.",
          });
        }
        if (p.includes("/later")) {
          // "Open Chrome in 20 seconds": approved now, runs later.
          const description = "Open Chrome in 20 seconds";
          const ok = await propose("open_app", description, 20);
          audit("open_app", "app=chrome", ok ? "done" : "declined", ok);
          onAction?.({
            type: "actionResult",
            kind: "open_app",
            description,
            outcome: ok ? "scheduled" : "declined",
            message: ok ? "Scheduled: it will run in 20 seconds." : "You chose not to do this.",
          });
        }
        if (p.includes("/remember")) {
          const content = "Prefers concise answers";
          const description = `Remember: "${content}"`;
          const ok = await propose("remember", description);
          if (ok) {
            memories = [
              {
                id: `mem-${Date.now()}`,
                content,
                category: "preference",
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                userApproved: true,
                enabled: true,
              },
              ...memories,
            ];
            listeners.data.forEach((cb) => cb("memory"));
          }
          audit("remember", "chars=23", ok ? "done" : "declined", ok);
          onAction?.({
            type: "actionResult",
            kind: "remember",
            description,
            outcome: ok ? "done" : "declined",
            message: ok ? "Saved to memory." : "You chose not to do this.",
          });
        }
        if (p.includes("/remind")) {
          const description = 'Remind you "Stretch" in an hour';
          const ok = await propose("create_reminder", description);
          if (ok) {
            const due = new Date(Date.now() + 3600_000).toISOString();
            const t = makeTask({ title: "Stretch", dueAt: due });
            tasks = [...tasks, t];
            listeners.data.forEach((cb) => cb("tasks"));
          }
          audit("create_reminder", 'title="Stretch"', ok ? "done" : "declined", ok);
          onAction?.({
            type: "actionResult",
            kind: "create_reminder",
            description,
            outcome: ok ? "done" : "declined",
            message: ok ? "Reminder created." : "You chose not to do this.",
          });
        }
        const text = mockAnswer(req.prompt);
        const words = text.split(/(\s+)/);
        let streamed = "";
        for (const w of words) {
          await sleep(tokenDelay, ctl.signal);
          streamed += w;
          onDelta(w);
        }
        // "/fallback" simulates the chosen model being used up for today.
        const fellBack = p.includes("/fallback");
        const answered = fellBack ? "gemini-2.5-flash-lite" : settings.selectedModel;
        const known = modelUse.find((u) => u.model === answered);
        if (known) known.requestsToday += 1;
        else
          modelUse.push({
            model: answered,
            label: answered === "gemini-2.5-flash-lite" ? "Gemini 2.5 Flash Lite" : answered,
            provider: "gemini",
            requestsToday: 1,
          });
        if (fellBack && !modelUse.some((u) => u.model === settings.selectedModel))
          modelUse.push({
            model: settings.selectedModel,
            label: settings.selectedModel,
            provider: "gemini",
            requestsToday: 20,
            restingUntil: Math.floor(Date.now() / 1000) + 6 * 3600,
          });
        return {
          text: streamed,
          model: answered,
          modelLabel: fellBack ? "Gemini 2.5 Flash Lite" : answered,
          fellBack,
          skipped: fellBack ? ["Gemini 2.5 Flash"] : [],
          note: null,
          latencyMs: Date.now() - started,
          usage: null,
          finishReason: "STOP",
          sources: req.web
            ? [
                { title: "Wikipedia", url: "https://en.wikipedia.org/wiki/Will-o%27-the-wisp" },
                { title: "Example News", url: "https://example.com/news" },
              ]
            : [],
        };
      })();
      const result = run.then(
        (r) => {
          logRequest(req.prompt, req.contextKind, Date.now() - started, null, r.text);
          return r;
        },
        (e: WispError) => {
          logRequest(req.prompt, req.contextKind, Date.now() - started, e.code, null);
          throw e;
        },
      );
      return { result, cancel: () => ctl.abort() };
    },

    // Like the host: a peek request never shrinks an open bar.
    setIslandMode: async (mode, height) =>
      mode === "peek" && layout.mode === "open" ? layout : setLayout(mode, height),
    onIslandLayout: (cb) => on(listeners.layout, cb),
    hideIsland: async () => {
      setLayout("sliver");
    },
    setTucked: async (tucked) => {
      settings = { ...settings, tucked };
      save();
      listeners.tucked.forEach((cb) => cb(tucked));
    },
    onTucked: (cb) => on(listeners.tucked, cb),
    onBlur: (cb) => on(listeners.blur, cb),
    prewarm: () => {},
    onHold: (cb) => on(listeners.hold, cb),
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

    startCapture: async () => {
      if (!settings.screenContextEnabled) {
        const e = err(
          "permission_denied",
          "Select to ask is turned off. Turn it on in Settings → Privacy.",
        );
        listeners.captureError.forEach((cb) => cb(e));
        return;
      }
      const svg = sampleCaptureSvg(480, 270);
      const ready: CaptureReady = {
        dataUrl: `data:image/svg+xml;base64,${btoa(svg)}`,
        mimeType: "image/svg+xml",
        dataBase64: btoa(svg),
        width: 480,
        height: 270,
      };
      setTimeout(() => listeners.captureReady.forEach((cb) => cb(ready)), 50);
    },
    overlayMonitor: () => Number(params.get("monitor") ?? 0),
    capturePreview: async () => `data:image/svg+xml;base64,${btoa(sampleCaptureSvg(1600, 900))}`,
    captureSelect: async (_monitor, rect) => {
      if (Math.abs(rect.width) < 8 || Math.abs(rect.height) < 8)
        throw err("invalid", "That selection is too small. Drag to select an area.");
    },
    captureSelectLasso: async (_monitor, points) => {
      if (points.length < 3) throw err("invalid", "Draw around the area you want to ask about");
    },
    captureCancel: async () => {},
    captureOverlayReady: () => {},
    reverseImageSearch: async () => ({ opened: true, pasted: false }),
    onCaptureSession: () => () => {},
    showMe: async (question) => {
      if (!hasKey)
        throw err("missing_key", "Add your Gemini API key in Settings → Model to ask Wisp.");
      if (!settings.screenContextEnabled)
        throw err(
          "permission_denied",
          "Screen features are turned off. Turn them on in Settings → Privacy.",
        );
      await new Promise((r) => setTimeout(r, 300));
      return question.toLowerCase().includes("nothing")
        ? []
        : [{ label: "Settings button", x: 0.86, y: 0.02, width: 0.05, height: 0.05 }];
    },
    highlightsGet: async () => [
      { label: "Settings button", x: 0.86, y: 0.04, width: 0.06, height: 0.06 },
    ],
    onCaptureReady: (cb) => on(listeners.captureReady, cb),
    onCaptureError: (cb) => on(listeners.captureError, cb),

    // Windows speech recognition, simulated: the words arrive one by one,
    // then listening ends after a short pause.
    voiceStart: async (target) => {
      const text = opts.voice ?? (persist ? "What's the weather like tomorrow?" : "");
      const words = text.split(" ").filter(Boolean);
      if (speechTimer) clearInterval(speechTimer);
      let n = 0;
      speechTarget = target;
      speechTimer = setInterval(() => {
        if (n < words.length) {
          n += 1;
          speechHeard = words.slice(0, n).join(" ");
          listeners.speech.forEach((cb) => cb({ target, text: speechHeard }));
        } else endSpeech();
      }, 60);
    },
    voiceStop: async () => endSpeech(),
    voiceCancel: async () => {
      if (speechTimer) clearInterval(speechTimer);
      speechTimer = null;
    },
    onSpeech: (cb) => on(listeners.speech, cb),
    onSpeechDone: (cb) => on(listeners.speechDone, cb),

    respondAction: async (id, approved) => {
      pendingActions.get(id)?.(approved);
    },
    tasksList: async (includeCompleted) =>
      [...tasks]
        .filter((t) => includeCompleted || !t.completedAt)
        .sort((a, b) => (a.dueAt ?? "9").localeCompare(b.dueAt ?? "9")),
    taskCreate: async (t) => {
      const task = makeTask(t);
      tasks = [...tasks, task];
      taskChanged();
      return task;
    },
    taskUpdate: async (id, t) => {
      const base = makeTask(t);
      return replaceTask({
        ...findTask(id),
        title: base.title,
        notes: base.notes,
        dueAt: base.dueAt,
        notifiedAt: null,
      });
    },
    taskComplete: async (id, completed) =>
      replaceTask({ ...findTask(id), completedAt: completed ? new Date().toISOString() : null }),
    taskSnooze: async (id, until) =>
      replaceTask({ ...findTask(id), dueAt: until, notifiedAt: null, completedAt: null }),
    taskDelete: async (id) => {
      const before = tasks.length;
      tasks = tasks.filter((t) => t.id !== id);
      taskChanged();
      return tasks.length < before;
    },
    onReminderDue: (cb) => on(listeners.reminder, cb),
    memoryList: async () => memories,
    memoryAdd: async (content, category) => {
      const c = content.trim();
      if (!c || c.length > 500) throw err("invalid", "Memory must be 1–500 characters");
      const m: MemoryEntry = {
        id: `mem-${Date.now()}-${memories.length}`,
        content: c,
        category,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        userApproved: true,
        enabled: true,
      };
      memories = [m, ...memories];
      listeners.data.forEach((cb) => cb("memory"));
      return m;
    },
    memoryUpdate: async (id, content, category, enabled) => {
      const c = content.trim();
      if (!c || c.length > 500) throw err("invalid", "Memory must be 1–500 characters");
      const m = memories.find((x) => x.id === id);
      if (!m) throw err("invalid", "That memory no longer exists");
      const next = { ...m, content: c, category, enabled, updatedAt: new Date().toISOString() };
      memories = memories.map((x) => (x.id === id ? next : x));
      listeners.data.forEach((cb) => cb("memory"));
      return next;
    },
    memoryDelete: async (id) => {
      memories = memories.filter((m) => m.id !== id);
      listeners.data.forEach((cb) => cb("memory"));
      return true;
    },
    clipboardRead: async () => {
      try {
        const t = await navigator.clipboard?.readText();
        if (t?.trim()) return t;
      } catch {
        /* permission denied in the preview */
      }
      return "Wisp is a small, spirit-like AI companion for Windows. It lives in a slim notch at the top of the screen and helps with quick questions, screen regions, dictation and simple tasks, while keeping data local by default.";
    },

    installedApps: async () => [
      {
        name: "spotify",
        target: "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Spotify.lnk",
      },
      {
        name: "visual studio code",
        target: "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Visual Studio Code.lnk",
      },
      {
        name: "word",
        target: "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Word.lnk",
      },
    ],
    checkForUpdates: async () => ({
      current: `${pkg.version}-preview`,
      latest: "0.1.0",
      available: false,
      url: null,
    }),
    uiReady: () => {},
    onIslandVisibility: () => () => {},

    historyList: async (limit = 200) => requests.slice(0, limit),
    actionsList: async (limit = 200) => actions.slice(0, limit),
    dataStats: async () => stats(),
    dataDelete: async (scope) => {
      if (scope === "history" || scope === "everything") {
        requests = [];
        actions = [];
      }
      if (scope === "memory" || scope === "everything") memories = [];
      if (scope === "tasks" || scope === "everything") tasks = [];
      if (scope === "clipboard" || scope === "everything") clips = [];
      if (scope === "settings" || scope === "everything") {
        settings = {
          ...DEFAULT_SETTINGS,
          onboardingComplete: scope === "settings" && settings.onboardingComplete,
        };
      }
      if (scope === "everything") {
        hasKey = false;
        if (persist) safeSet(KEY_FLAG, null);
      }
      save();
      listeners.data.forEach((cb) => cb(scope));
      return stats();
    },
    onDataChanged: (cb) => on(listeners.data, cb),
    diagnosticsReport: async () =>
      JSON.stringify(
        {
          generatedAt: new Date().toISOString(),
          appVersion: `${pkg.version}-preview`,
          os: "browser-preview",
          hasApiKey: hasKey,
          settings: {
            ...settings,
            allowedApps: settings.allowedApps.length,
            windowPosition: undefined,
          },
          storage: stats(),
          recentRequests: requests.slice(0, 50).map((r) => ({
            timestamp: r.timestamp,
            type: r.requestType,
            model: r.model,
            status: r.status,
            latencyMs: r.latencyMs,
            errorCode: r.errorCode,
            contextKind: r.contextKind,
          })),
        },
        null,
        2,
      ),
    diagnosticsSave: async () => "Downloads/wisp-diagnostics-preview.json",

    openExternal: async (url) => {
      window.open(url, "_blank", "noopener");
    },
    copyText: async (text) => {
      await navigator.clipboard?.writeText(text);
    },

    controlsState: async () => controls,
    nowPlaying: async () => controls.nowPlaying,
    setVolume: async (percent) => {
      controls = { ...controls, volume: Math.round(percent), muted: percent === 0 };
    },
    setMuted: async (muted) => {
      controls = { ...controls, muted };
    },
    mediaCommand: async (command) => {
      if (!controls.nowPlaying) throw err("unsupported", "Nothing is playing right now.");
      if (command === "playPause")
        controls = {
          ...controls,
          nowPlaying: { ...controls.nowPlaying, playing: !controls.nowPlaying.playing },
        };
    },
    setRadio: async (kind, on) => {
      controls = {
        ...controls,
        radios: controls.radios.map((r) => (r.kind === kind ? { ...r, on } : r)),
      };
    },
    setDarkMode: async (dark) => {
      controls = { ...controls, darkMode: dark };
    },
    statsSnapshot: async () => {
      const wobble = (base: number) =>
        Math.max(0, Math.min(100, base + (Math.random() - 0.5) * 20));
      const cores = Array.from({ length: 8 }, (_, i) => wobble(15 + i * 6));
      return {
        cpu: cores.reduce((a, b) => a + b, 0) / cores.length,
        cores,
        cpuName: "Preview CPU @ 3.6GHz",
        memoryTotal: 16 * 1024 ** 3,
        memoryUsed: 9.4 * 1024 ** 3,
        disks: [{ mount: "C:\\", totalBytes: 512e9, freeBytes: 188e9 }],
        netDown: 1.8e6 + Math.random() * 4e5,
        netUp: 2.2e5,
        uptimeSecs: 3 * 86400 + 4 * 3600,
        top: [
          { name: "chrome", count: 14, memoryBytes: 2.1e9, cpu: 6.2 },
          { name: "Code", count: 6, memoryBytes: 1.2e9, cpu: 3.1 },
          { name: "Discord", count: 4, memoryBytes: 6.1e8, cpu: 0.8 },
          { name: "explorer", count: 1, memoryBytes: 2.2e8, cpu: 0.2 },
          { name: "wisp", count: 1, memoryBytes: 9.0e7, cpu: 0.1 },
        ],
      };
    },
    speedTest: async () => {
      await new Promise((r) => setTimeout(r, 900));
      return { mbps: 212.4, bytes: 25_000_000, millis: 940, latencyMs: 18 };
    },
    weatherSearch: async (query) => {
      if (query.trim().length < 2)
        throw err("invalid", "Type at least two letters of a town or city");
      return [
        { name: "London, England, United Kingdom", latitude: 51.5, longitude: -0.12 },
        { name: "London, Ontario, Canada", latitude: 42.98, longitude: -81.24 },
      ];
    },
    weatherGet: async () =>
      settings.weatherPlace
        ? {
            place: settings.weatherPlace.name,
            temperature: 14.2,
            windKmh: 11,
            code: 2,
            label: "Partly cloudy",
            isDay: true,
            days: ["Mon", "Tue", "Wed", "Thu", "Fri"].map((_, i) => ({
              date: new Date(Date.now() + i * 86400_000).toISOString().slice(0, 10),
              min: 8 + i,
              max: 15 + (i % 3),
              code: [2, 61, 3, 0, 80][i]!,
              label: ["Partly cloudy", "Rain", "Overcast", "Clear", "Showers"][i]!,
            })),
          }
        : null,
    clipsList: async (k) =>
      [...clips]
        .filter((c) => !k || c.kind === k)
        .sort((a, b) =>
          a.pinned === b.pinned ? b.createdAt.localeCompare(a.createdAt) : a.pinned ? -1 : 1,
        ),
    clipPin: async (id, pinned) => {
      clips = clips.map((c) => (c.id === id ? { ...c, pinned } : c));
      clipsChanged();
      return true;
    },
    clipDelete: async (id) => {
      clips = clips.filter((c) => c.id !== id);
      clipsChanged();
      return true;
    },
    clipsClear: async (includePinned) => {
      const before = clips.length;
      clips = clips.filter((c) => c.pinned && !includePinned);
      clipsChanged();
      return before - clips.length;
    },
    clipCopy: async (id) => {
      const c = clips.find((x) => x.id === id);
      if (c) await navigator.clipboard?.writeText(c.text).catch(() => {});
    },
    launcherList: async () => ({ items: catalogue, recent: usage }),
    launchItem: async (item) => {
      if (!catalogue.some((c) => c.id === item.id))
        throw err("invalid", "That launcher entry doesn't match its target");
      const prev = usage.find((u) => u.id === item.id);
      usage = [
        { id: item.id, lastUsed: new Date().toISOString(), launches: (prev?.launches ?? 0) + 1 },
        ...usage.filter((u) => u.id !== item.id),
      ];
    },
    powerAction: async () => {},
    wallpaperInfo: async () => ({ current: currentWall, recent: settings.recentWallpapers }),
    wallpaperThumb: async () => `data:image/svg+xml;base64,${btoa(sampleCaptureSvg(320, 180))}`,
    wallpaperPick: async () => {
      const path = `C:\\Users\\you\\Pictures\\wallpaper-${settings.recentWallpapers.length + 1}.jpg`;
      settings = {
        ...settings,
        recentWallpapers: [path, ...settings.recentWallpapers].slice(0, 12),
      };
      save();
      return path;
    },
    wallpaperList: async () => folderImages(settings.wallpaperFolders),
    wallpaperFolderAdd: async () => {
      const f = `C:\\Users\\you\\Pictures\\Wallpapers${settings.wallpaperFolders.length || ""}`;
      settings = { ...settings, wallpaperFolders: [...settings.wallpaperFolders, f] };
      save();
      return settings.wallpaperFolders;
    },
    wallpaperFolderRemove: async (folder) => {
      settings = {
        ...settings,
        wallpaperFolders: settings.wallpaperFolders.filter((f) => f !== folder),
      };
      save();
      return settings.wallpaperFolders;
    },
    wallpaperNext: async (shuffle) => {
      const list = folderImages(settings.wallpaperFolders);
      if (!list.length)
        throw err("invalid", "Add a wallpaper folder first (Wallpaper → Add folder).");
      const at = currentWall ? list.indexOf(currentWall) : -1;
      const i = shuffle ? (at + 3) % list.length : (at + 1) % list.length;
      currentWall = list[i] ?? null;
      return currentWall;
    },
    winSettings: async () => win,
    winSettingSet: async (change) => {
      switch (change.setting) {
        case "darkMode":
          win = { ...win, appsDark: change.value, systemDark: change.value };
          controls = { ...controls, darkMode: change.value };
          break;
        case "outputDevice": {
          const q = change.value.toLowerCase();
          const d = win.outputs.find(
            (o) => o.id === change.value || o.name.toLowerCase().includes(q),
          );
          if (!d) throw err("invalid", `No sound output matches "${change.value}".`);
          win = { ...win, outputs: win.outputs.map((o) => ({ ...o, default: o.id === d.id })) };
          return { message: `Sound now plays through ${d.name}.` };
        }
        case "micMute":
          win = { ...win, micMuted: change.value };
          break;
        default:
          win = { ...win, [change.setting]: change.value };
      }
      return { message: "Done." };
    },
    winSettingsPage: async () => "ms-settings:",
    scheduledList: async () => [],
    scheduledCancel: async () => false,
    onScheduledRan: (cb) => on(scheduledListeners, cb),
    wallpaperSet: async (path) => {
      currentWall = path;
      settings = {
        ...settings,
        recentWallpapers: [path, ...settings.recentWallpapers.filter((p) => p !== path)],
      };
      save();
    },
    agentStart: async (k) => {
      if (!hasKey)
        throw err("missing_key", "Add your Gemini API key in Settings → Model to ask Wisp.");
      const job: AgentJob = {
        id: `job-${Date.now()}`,
        kind: k.kind,
        title: k.kind === "research" ? `Research: ${k.topic}` : "Recreate this screen as HTML",
        status: "running",
        progress: 0.1,
        detail: k.kind === "research" ? "Searching the web…" : "Studying the layout…",
        outputPath: null,
        createdAt: new Date().toISOString(),
        finishedAt: null,
      };
      jobs = [job, ...jobs];
      jobsChanged();
      setTimeout(() => {
        jobs = jobs.map((j) =>
          j.id === job.id && j.status === "running"
            ? {
                ...j,
                status: "done",
                progress: 1,
                detail: "Saved",
                outputPath: `Documents\\Wisp\\${k.kind === "research" ? "brief.md" : "replica.html"}`,
                finishedAt: new Date().toISOString(),
              }
            : j,
        );
        jobsChanged();
      }, 1500);
      return job;
    },
    agentCancel: async (id) => {
      jobs = jobs.map((j) =>
        j.id === id && j.status === "running"
          ? { ...j, status: "cancelled", detail: "Cancelled", finishedAt: new Date().toISOString() }
          : j,
      );
      jobsChanged();
    },
    agentsList: async () => jobs,
    agentDelete: async (id) => {
      jobs = jobs.filter((j) => j.id !== id);
      jobsChanged();
      return true;
    },
    agentOpen: async () => {},
    foregroundApp: async () => "code",
    openWindows: async () => [
      { id: 1, app: "chrome", title: "Quarterly report - Google Docs" },
      { id: 2, app: "code", title: "notch.css - WISP - Visual Studio Code" },
      { id: 3, app: "spotify", title: "Spotify Premium" },
    ],
    switchWindow: async (id) => {
      if (![1, 2, 3].includes(id)) throw err("invalid", "That window isn't open any more");
      setLayout("sliver");
    },
    screenStatus: async () => "show",

    pauseShortcuts: () => {},
    reportError: (message) => {
      errors.push(message);
    },
    onSelfTestRun: (cb) => on(listeners.selfTest, cb),
    selfTestProbe: async () => ({
      mode: layout.mode,
      visible: true,
      x: 0,
      y: 0,
      width: layout.mode === "open" ? 680 : layout.mode === "peek" ? 400 : 180,
      height: layout.mode === "open" ? 360 : layout.mode === "peek" ? 58 : 10,
      settingsOpen: false,
      overlays: 0,
      focused: layout.mode === "open",
    }),
    selfTestAction: async (action) => {
      if (action === "summon") {
        setLayout("open");
        listeners.focus.forEach((cb) => cb());
      }
      if (action === "close") setLayout("sliver");
    },
    selfTestReport: async (section, steps) => {
      selfTestReports.push({ section, steps });
    },
  };
}
