// Mirrors of the Rust types that cross the IPC boundary (camelCase).

export type ThemePreference = "system" | "dark" | "light";
export type MotionPreference = "system" | "reduced" | "full";

export interface Shortcuts {
  toggleIsland: string;
  selectToAsk: string;
  dictation: string;
  clipboardAssist: string;
  voice: string;
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
  windowPosition: { x: number; y: number } | null;
  globalShortcuts: Shortcuts;
  selectedModel: string;
  voiceEnabled: boolean;
  speakResponses: boolean;
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
}

export interface IslandLayout {
  expanded: boolean;
  alignRight: boolean;
  alignBottom: boolean;
}

export const DEFAULT_SHORTCUTS: Shortcuts = {
  toggleIsland: "Ctrl+Shift+Space",
  selectToAsk: "Ctrl+Shift+S",
  dictation: "Ctrl+Shift+D",
  clipboardAssist: "Ctrl+Shift+C",
  voice: "Ctrl+Shift+V",
};

export const DEFAULT_SETTINGS: Settings = {
  onboardingComplete: false,
  launchAtLogin: false,
  alwaysOnTop: true,
  startHidden: false,
  islandVisible: true,
  theme: "system",
  motion: "system",
  windowPosition: null,
  globalShortcuts: DEFAULT_SHORTCUTS,
  selectedModel: "gemini-2.5-flash",
  voiceEnabled: true,
  speakResponses: false,
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
  schemaVersion: number;
}

export type DeleteScope = "history" | "memory" | "tasks" | "settings" | "everything";

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
