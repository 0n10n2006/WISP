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
  /** Skip the model's thinking step for much faster answers. */
  fastMode: boolean;
  dictationStyle: DictationStyle;
  /** Hold Ctrl+Win to dictate, Win+Alt to ask by voice (keys configurable). */
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
  /** `#rrggbb`, used when `accent` is "custom". */
  accentColor: string;
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
  /** Stop listening automatically when you finish speaking. */
  autoStopVoice: boolean;
  /** Pause that counts as "done talking", in ms. */
  silenceMs: number;
  /** Language hint such as "en-GB"; empty = detect. */
  voiceLanguage: string;
  /** Microphone device id; empty = default. */
  micDevice: string;
  /** Names and words dictation should get right. */
  vocabulary: string[];
  hideInGames: boolean;
  shortcutsInGames: boolean;
  /** Process names Wisp never appears over. */
  neverOverApps: string[];
  appAwareness: boolean;
  shareWindowTitles: boolean;
  /** Forget history whenever Wisp closes or Windows shuts down. */
  clearHistoryOnExit: boolean;
  screenContextEnabled: boolean;
  dictationAutoInsert: boolean;
  retainAudio: boolean;
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

export type DictationStyle = "verbatim" | "clean" | "formal";
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
  fastMode: true,
  dictationStyle: "clean",
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
  accentColor: "#78e4f2",
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
  autoStopVoice: true,
  silenceMs: 1200,
  voiceLanguage: "",
  micDevice: "",
  vocabulary: [],
  hideInGames: true,
  shortcutsInGames: false,
  neverOverApps: [],
  appAwareness: true,
  shareWindowTitles: false,
  clearHistoryOnExit: false,
  screenContextEnabled: true,
  dictationAutoInsert: false,
  retainAudio: false,
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
  outcome: "done" | "rejected" | "declined" | "failed";
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
      outcome: "done" | "declined" | "rejected" | "failed";
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
  phase: "start" | "stop" | "cancel";
}

export interface OpenWindow {
  id: number;
  /** Process name, e.g. "chrome". */
  app: string;
  title: string;
}

/** Why the notch is hidden right now. */
export type Cover = "show" | "fullscreen" | "game" | "presentation" | "excluded";
