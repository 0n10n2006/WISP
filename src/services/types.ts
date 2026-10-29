// Mirrors of the Rust types that cross the IPC boundary (camelCase).

export type ThemePreference = "system" | "dark" | "light";
export type MotionPreference = "system" | "reduced" | "full";

export interface Shortcuts {
  toggleIsland: string;
  selectToAsk: string;
  dictation: string;
  clipboardAssist: string;
  voice: string;
  tuck: string;
}

export type ShortcutAction = keyof Shortcuts;

export interface AllowedApp {
  name: string;
  target: string;
}

export interface Settings {
  onboardingComplete: boolean;
  launchAtLogin: boolean;
  alwaysOnTop: boolean;
  startHidden: boolean;
  islandVisible: boolean;
  theme: ThemePreference;
  motion: MotionPreference;
  islandScale: number;
  windowPosition: { x: number; y: number } | null;
  globalShortcuts: Shortcuts;
  selectedModel: string;
  voiceEnabled: boolean;
  /** Soft sounds when Wisp starts and stops listening. */
  voiceSounds: boolean;
  /** Skip the model's thinking step for much faster answers. */
  fastMode: boolean;
  /** Tap Ctrl+Win to dictate, Win+Alt to ask by voice (keys configurable). */
  holdToTalk: boolean;
  /** Modifier pair held to dictate, e.g. "ctrl+win", or "off". */
  holdDictate: HoldChord;
  /** Modifier pair held to ask by voice, e.g. "win+alt", or "off". */
  holdAsk: HoldChord;
  autoHideFullscreen: boolean;
  tucked: boolean;
  clipboardHistory: boolean;
  weatherPlace: WeatherPlace | null;
  persona: Persona;
  accent: Accent;
  starredApps: string[];
  focusMinutes: number;
  recentWallpapers: string[];
  /** Folders the wallpaper carousel shows (chosen in a folder picker). */
  wallpaperFolders: string[];
  /** `#rrggbb`, used when `accent` is "custom". */
  accentColor: string;
  /** Keep an index of file names in the user's folders for instant search. */
  indexFiles: boolean;
  /** Extra folders to index (chosen in a folder picker). */
  indexFolders: string[];
  /** Search open browser tabs, history and bookmarks from the bar. */
  browserSearch: boolean;
  /** "g cats" style site searches. */
  webShortcuts: WebShortcut[];
  notchPosition: NotchPosition;
  sliverStyle: SliverStyle;
  /** A notch along the top, or a floating ball in a corner. */
  notchStyle: NotchStyle;
  ballCorner: BallCorner;
  /** Dashboard tabs to show, in order. */
  dashboardTabs: string[];
  closeOnBlur: boolean;
  /** Fold the bar away shortly after the mouse leaves it. */
  foldOnLeave: boolean;
  /** When a model's free quota runs out, answer with the next one. */
  autoFallback: boolean;
  /** Send simple jobs to OpenRouter first (with a key). */
  simpleViaOpenrouter: boolean;
  /** OpenRouter model; "" picks the biggest free one. */
  openrouterModel: string;
  /** Ask before acting: only when risky, or always. */
  confirmActions: "risky" | "always";
  /** Settings migration level (host-managed). */
  settingsRev?: number;
  hideInGames: boolean;
  shortcutsInGames: boolean;
  /** Process names Wisp never appears over. */
  neverOverApps: string[];
  appAwareness: boolean;
  shareWindowTitles: boolean;
  /** Forget history whenever Wisp closes or Windows shuts down. */
  clearHistoryOnExit: boolean;
  screenContextEnabled: boolean;
  useMemory: boolean;
  contentHistory: boolean;
  historyRetentionDays: number;
  notificationsEnabled: boolean;
  telemetryOptIn: boolean;
  allowedApps: AllowedApp[];
}

export type HoldChord =
  "ctrl+win" | "win+alt" | "ctrl+alt" | "ctrl+shift" | "alt+shift" | "win+shift" | "off";

export const HOLD_CHOICES: HoldChord[] = [
  "ctrl+win",
  "win+alt",
  "ctrl+alt",
  "ctrl+shift",
  "alt+shift",
  "win+shift",
  "off",
];

/** "ctrl+win" → "Ctrl+Win". */
export function chordLabel(c: HoldChord): string {
  if (c === "off") return "Off";
  return c
    .split("+")
    .map((p) => p[0]!.toUpperCase() + p.slice(1))
    .join("+");
}

export type Tone = "concise" | "friendly" | "professional";
export type Accent = "cyan" | "lavender" | "ember" | "mint" | "custom";
export type NotchPosition = "left" | "center" | "right";
export type SliverStyle = "glow" | "subtle" | "hidden";
export type NotchStyle = "notch" | "ball";
export type BallCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

export interface Persona {
  tone: Tone;
  instructions: string;
}

export interface WeatherPlace {
  name: string;
  latitude: number;
  longitude: number;
}

export interface ShortcutIssue {
  action: ShortcutAction;
  accelerator: string;
  message: string;
}

export interface SettingsSnapshot {
  settings: Settings;
  hasApiKey: boolean;
  /** An OpenRouter key is saved (the key itself never reaches the UI). */
  hasOpenrouterKey?: boolean;
  shortcutIssues: ShortcutIssue[];
  recoveredFrom: string | null;
  warnings: string[];
  version: string;
}

export type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? (T[K] extends unknown[] ? T[K] : DeepPartial<T[K]>) : T[K];
};

export interface ModelInfo {
  id: string;
  displayName: string;
  description: string;
  inputTokenLimit: number;
}

export interface ConnectionReport {
  ok: boolean;
  modelAvailable: boolean;
  models: ModelInfo[];
  latencyMs: number;
}

export type ContextKind = "text" | "screen" | "clipboard" | "voice" | "dictation";

export interface ChatTurn {
  role: "user" | "model";
  text: string;
}

export interface Attachment {
  mimeType: string;
  dataBase64: string;
}

export interface AskRequest {
  requestId: string;
  prompt: string;
  history: ChatTurn[];
  attachments: Attachment[];
  contextKind: ContextKind;
  useMemory: boolean;
  enableTools: boolean;
  /** Let the model think longer (slower, better for hard questions). */
  think?: boolean;
  /** Ground the answer with Google Search. */
  web?: boolean;
  /** An earlier turn included screen, clipboard or web content. */
  outsideContent?: boolean;
}

export interface Source {
  title: string;
  url: string;
}

export interface Usage {
  promptTokenCount: number;
  candidatesTokenCount: number;
  totalTokenCount: number;
}

export interface AskResult {
  text: string;
  model: string;
  latencyMs: number;
  usage: Usage | null;
  finishReason: string | null;
  sources: Source[];
  /** Readable name of the model that answered. */
  modelLabel?: string;
  /** Another model answered because the first choice was used up or busy. */
  fellBack?: boolean;
  /** Models skipped on the way. */
  skipped?: string[];
  /** Something to know about this answer (e.g. no web search). */
  note?: string | null;
}

/** A free OpenRouter model. */
export interface FreeModel {
  id: string;
  name: string;
  contextLength: number;
}

/** Today's use of one model, and whether it's resting after a limit. */
export interface ModelUsage {
  model: string;
  label: string;
  provider: "gemini" | "openrouter";
  requestsToday: number;
  /** Unix seconds when it can be used again. */
  restingUntil?: number | null;
}

export type NotchMode = "sliver" | "peek" | "open";

export interface IslandLayout {
  mode: NotchMode;
  expanded: boolean;
  height: number;
}

export const DEFAULT_SHORTCUTS: Shortcuts = {
  toggleIsland: "Ctrl+Alt+Space",
  selectToAsk: "Ctrl+Shift+S",
  dictation: "Ctrl+Shift+D",
  clipboardAssist: "Ctrl+Shift+C",
  voice: "Ctrl+Alt+V",
  tuck: "Ctrl+Alt+H",
};

export const DEFAULT_SETTINGS: Settings = {
  onboardingComplete: false,
  launchAtLogin: false,
  alwaysOnTop: true,
  startHidden: false,
  islandVisible: true,
  theme: "system",
  motion: "system",
  islandScale: 1,
  windowPosition: null,
  globalShortcuts: DEFAULT_SHORTCUTS,
  selectedModel: "gemini-2.5-flash",
  voiceEnabled: true,
  voiceSounds: true,
  fastMode: true,
  holdToTalk: true,
  holdDictate: "ctrl+win",
  holdAsk: "win+alt",
  autoHideFullscreen: true,
  tucked: false,
  clipboardHistory: false,
  weatherPlace: null,
  persona: { tone: "concise", instructions: "" },
  accent: "cyan",
  starredApps: [],
  focusMinutes: 25,
  recentWallpapers: [],
  wallpaperFolders: [],
  accentColor: "#78e4f2",
  indexFiles: true,
  indexFolders: [],
  browserSearch: true,
  webShortcuts: [
    { prefix: "g", name: "Google", url: "https://www.google.com/search?q=%s" },
    { prefix: "yt", name: "YouTube", url: "https://www.youtube.com/results?search_query=%s" },
    { prefix: "wiki", name: "Wikipedia", url: "https://en.wikipedia.org/w/index.php?search=%s" },
    { prefix: "maps", name: "Google Maps", url: "https://www.google.com/maps/search/%s" },
    { prefix: "gh", name: "GitHub", url: "https://github.com/search?q=%s" },
    { prefix: "ddg", name: "DuckDuckGo", url: "https://duckduckgo.com/?q=%s" },
    { prefix: "amz", name: "Amazon", url: "https://www.amazon.com/s?k=%s" },
    { prefix: "r", name: "Reddit", url: "https://www.reddit.com/search/?q=%s" },
  ],
  notchPosition: "center",
  sliverStyle: "glow",
  notchStyle: "notch",
  ballCorner: "bottom-right",
  dashboardTabs: [
    "today",
    "controls",
    "stats",
    "clipboard",
    "apps",
    "agents",
    "wallpaper",
    "power",
  ],
  closeOnBlur: true,
  foldOnLeave: true,
  autoFallback: true,
  simpleViaOpenrouter: true,
  openrouterModel: "",
  confirmActions: "risky",
  hideInGames: true,
  shortcutsInGames: false,
  neverOverApps: [],
  appAwareness: true,
  shareWindowTitles: false,
  clearHistoryOnExit: false,
  screenContextEnabled: true,
  useMemory: true,
  contentHistory: false,
  historyRetentionDays: 30,
  notificationsEnabled: true,
  telemetryOptIn: false,
  allowedApps: [
    { name: "notepad", target: "notepad.exe" },
    { name: "calculator", target: "calc.exe" },
    { name: "paint", target: "mspaint.exe" },
    { name: "file explorer", target: "explorer.exe" },
  ],
};

export interface RequestLog {
  id: string;
  timestamp: string;
  requestType: string;
  model: string;
  status: "ok" | "error" | "cancelled";
  latencyMs: number;
  errorCode: string | null;
  contextKind: string;
  totalTokens: number | null;
  prompt: string | null;
  response: string | null;
}

export interface ActionAudit {
  id: string;
  timestamp: string;
  actionType: string;
  summary: string;
  outcome: "done" | "scheduled" | "rejected" | "declined" | "failed";
  userConfirmed: boolean;
  detail: string | null;
}

export interface StorageStats {
  requests: number;
  actions: number;
  tasks: number;
  openTasks: number;
  memories: number;
  clips: number;
  schemaVersion: number;
}

export type DeleteScope = "history" | "memory" | "tasks" | "clipboard" | "settings" | "everything";

export interface CaptureReady {
  dataUrl: string;
  mimeType: string;
  dataBase64: string;
  width: number;
  height: number;
}

export interface SelectionRect {
  x: number;
  y: number;
  width: number;
  height: number;
  viewWidth: number;
  viewHeight: number;
}

export interface Task {
  id: string;
  title: string;
  notes: string | null;
  dueAt: string | null;
  createdAt: string;
  completedAt: string | null;
  notificationEnabled: boolean;
  notifiedAt: string | null;
  repeat: Repeat;
}

export type Repeat = "none" | "daily" | "weekdays" | "weekly";

export interface NewTask {
  title: string;
  notes?: string | null;
  dueAt?: string | null;
  notificationEnabled?: boolean;
  repeat?: Repeat;
}

export type MemoryCategory = "preference" | "fact" | "project" | "people" | "other";

export interface MemoryEntry {
  id: string;
  content: string;
  category: MemoryCategory;
  createdAt: string;
  updatedAt: string;
  userApproved: boolean;
  enabled: boolean;
}

export type ActionEvent =
  | { type: "actionProposed"; id: string; kind: string; description: string }
  | {
      type: "actionResult";
      kind: string;
      description: string;
      outcome: "done" | "scheduled" | "declined" | "rejected" | "failed";
      message: string;
    };

export interface InstalledApp {
  name: string;
  target: string;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  available: boolean;
  url: string | null;
}

// ---- v0.2 dashboard ---------------------------------------------------------

export interface NowPlaying {
  title: string;
  artist: string;
  playing: boolean;
  app: string;
}

export interface RadioInfo {
  kind: "wifi" | "bluetooth";
  name: string;
  on: boolean;
}

export interface ControlsState {
  supported: boolean;
  volume: number | null;
  muted: boolean;
  nowPlaying: NowPlaying | null;
  radios: RadioInfo[];
  darkMode: boolean;
}

export type MediaCommand = "playPause" | "next" | "previous";

export type TaskbarAlign = "left" | "center";
export type PowerMode = "efficiency" | "balanced" | "performance";

export interface AudioDevice {
  id: string;
  name: string;
  default: boolean;
}

/** Windows settings for the control centre; null = unknown or unsupported. */
export interface WinSettingsState {
  supported: boolean;
  appsDark: boolean | null;
  systemDark: boolean | null;
  transparency: boolean | null;
  accentColor: string | null;
  accentFromWallpaper: boolean | null;
  accentOnTaskbar: boolean | null;
  taskbarAlign: TaskbarAlign | null;
  taskbarAutoHide: boolean | null;
  desktopIcons: boolean | null;
  startRecent: boolean | null;
  brightness: number | null;
  nightLight: boolean | null;
  doNotDisturb: boolean | null;
  powerMode: PowerMode | null;
  outputs: AudioDevice[];
  micMuted: boolean | null;
}

/** One change to a Windows setting (mirrors the host's `WinChange`). */
export type WinChange =
  | { setting: "darkMode" | "appsDark" | "systemDark" | "transparency"; value: boolean }
  | { setting: "accentFromWallpaper" | "accentOnTaskbar" | "taskbarAutoHide"; value: boolean }
  | { setting: "desktopIcons" | "startRecent" | "nightLight" | "doNotDisturb"; value: boolean }
  | { setting: "micMute"; value: boolean }
  | { setting: "accentColor"; value: string }
  | { setting: "taskbarAlign"; value: TaskbarAlign }
  | { setting: "brightness"; value: number }
  | { setting: "powerMode"; value: PowerMode }
  | { setting: "outputDevice"; value: string };

export type VoiceTarget = "app" | "bar";

/** Words heard by Windows' speech recognizer. */
export interface Heard {
  target: VoiceTarget;
  text: string;
  /** Why listening ended early (no network, no microphone…). */
  error?: string;
}

/** A confirmed action waiting to run ("open Chrome in 20 seconds"). */
export interface ScheduledAction {
  id: string;
  description: string;
  /** Unix ms. */
  runAt: number;
}

export interface ProcessStat {
  name: string;
  count: number;
  memoryBytes: number;
  cpu: number;
}

export interface DiskStat {
  mount: string;
  totalBytes: number;
  freeBytes: number;
}

export interface StatsSnapshot {
  cpu: number;
  cores: number[];
  cpuName: string;
  memoryTotal: number;
  memoryUsed: number;
  disks: DiskStat[];
  netDown: number;
  netUp: number;
  uptimeSecs: number;
  top: ProcessStat[];
}

export type FileKind =
  | "folder"
  | "pdf"
  | "document"
  | "sheet"
  | "slides"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "other";

/** A file from the index. The host keeps the path; the UI opens by id. */
export interface FileHit {
  id: number;
  name: string;
  /** Where it is, e.g. "Downloads/Invoices". */
  folder: string;
  kind: FileKind;
  size: number;
  /** Unix ms. */
  modified: number;
}

export interface IndexFolder {
  path: string;
  label: string;
  /** Added by the user (not a standard Windows folder). */
  user: boolean;
}

export interface FilesStatus {
  enabled: boolean;
  indexing: boolean;
  count: number;
  scanned: number;
  /** When the last full pass finished (unix ms). */
  updatedAt: number | null;
  folders: IndexFolder[];
}

export interface WebShortcut {
  prefix: string;
  name: string;
  /** https address with %s where the words go. */
  url: string;
}

/** A tab open in a browser window. */
export interface BrowserTab {
  id: number;
  title: string;
  browser: string;
}

/** A page from browser history or bookmarks. */
export interface BrowserItem {
  id: number;
  title: string;
  url: string;
  browser: string;
  kind: "history" | "bookmark";
  visits: number;
  /** Unix ms (0 if never visited). */
  lastVisit: number;
}

/** Running programs, grouped by name. */
export interface ProcInfo {
  name: string;
  /** How many processes have this name. */
  count: number;
  /** The biggest one. */
  pid: number;
  memoryBytes: number;
  cpu: number;
  /** One of its windows, if it has any. */
  window: number | null;
  /** Wisp won't end it (Windows itself, or Wisp). */
  protected: boolean;
}

export interface FilePreview {
  kind: "image" | "text" | "folder" | "none";
  /** Data URL for images, PDFs, documents and videos. */
  image?: string;
  text?: string;
  entries?: string[];
}

/** Exchange rates (ECB, via frankfurter.dev). */
export interface CurrencyRates {
  base: string;
  date: string;
  rates: Record<string, number>;
}

export interface Battery {
  percent: number;
  charging: boolean;
  pluggedIn: boolean;
  secsLeft: number | null;
}

/** For questions answered on this PC ("battery", "disk space"). */
export interface PcInfo {
  battery: Battery | null;
  localIp: string | null;
  stats: StatsSnapshot;
}

export interface SpeedResult {
  mbps: number;
  bytes: number;
  millis: number;
  latencyMs: number;
}

export interface DayForecast {
  date: string;
  min: number;
  max: number;
  code: number;
  label: string;
}

export interface Weather {
  place: string;
  temperature: number;
  windKmh: number;
  code: number;
  label: string;
  isDay: boolean;
  days: DayForecast[];
}

export interface ClipEntry {
  id: string;
  kind: "text" | "link";
  text: string;
  createdAt: string;
  pinned: boolean;
}

export interface LaunchItem {
  id: string;
  name: string;
  kind: "app" | "game" | "system";
  target: string;
}

export interface AppUsage {
  id: string;
  lastUsed: string;
  launches: number;
}

export interface Catalogue {
  items: LaunchItem[];
  recent: AppUsage[];
}

export type PowerAction = "lock" | "sleep" | "restart" | "shutdown" | "cancel";

export interface WallpaperInfo {
  current: string | null;
  recent: string[];
}

export type AgentKind = { kind: "replica" } | { kind: "research"; topic: string };

export interface AgentJob {
  id: string;
  kind: "replica" | "research";
  title: string;
  status: "running" | "done" | "failed" | "cancelled";
  progress: number;
  detail: string | null;
  outputPath: string | null;
  createdAt: string;
  finishedAt: string | null;
}

export interface Highlight {
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface HoldEvent {
  chord: "dictate" | "ask";
  phase: "start" | "stop" | "cancel" | "tap";
}

export interface OpenWindow {
  id: number;
  /** Process name, e.g. "chrome". */
  app: string;
  title: string;
}

/** Why the notch is hidden right now. */
export type Cover = "show" | "fullscreen" | "game" | "presentation" | "excluded";
