// The single boundary between the UI and the desktop host.
// `tauriHost` talks to Rust; `mockHost` lets the whole UI run in a browser.

import type { WispError } from "./errors";
import type { AudioRecorder, Recording } from "./recorder";
import type {
  InstalledApp,
  UpdateInfo,
  ActionAudit,
  ActionEvent,
  MemoryCategory,
  MemoryEntry,
  NewTask,
  Task,
  CaptureReady,
  SelectionRect,
  AskRequest,
  DeleteScope,
  RequestLog,
  StorageStats,
  AskResult,
  ConnectionReport,
  DeepPartial,
  IslandLayout,
  ModelInfo,
  Settings,
  SettingsSnapshot,
  ShortcutAction,
  AgentJob,
  AgentKind,
  Catalogue,
  ClipEntry,
  ControlsState,
  Highlight,
  HoldEvent,
  LaunchItem,
  MediaCommand,
  NowPlaying,
  NotchMode,
  Point,
  PowerAction,
  SpeedResult,
  StatsSnapshot,
  WallpaperInfo,
  Weather,
  WeatherPlace,
} from "./types";

export type Unsubscribe = () => void;

export type WindowKind = "island" | "settings" | "overlay" | "highlight";

export interface AskHandle {
  result: Promise<AskResult>;
  cancel(): void;
}

export interface Host {
  readonly kind: "tauri" | "mock";
  windowKind(): WindowKind;
  initialTab(): string | null;

  getSettings(): Promise<SettingsSnapshot>;
  updateSettings(patch: DeepPartial<Settings>): Promise<SettingsSnapshot>;
  resetShortcuts(): Promise<SettingsSnapshot>;
  resetSettings(): Promise<SettingsSnapshot>;
  validateShortcut(accelerator: string): Promise<string>;
  onSettings(cb: (s: SettingsSnapshot) => void): Unsubscribe;

  setApiKey(key: string): Promise<SettingsSnapshot>;
  removeApiKey(): Promise<SettingsSnapshot>;
  testConnection(key?: string, model?: string): Promise<ConnectionReport>;
  listModels(): Promise<ModelInfo[]>;

  ask(
    req: AskRequest,
    onDelta: (text: string) => void,
    onAction?: (e: ActionEvent) => void,
  ): AskHandle;
  /** Answer a confirmation for a model-proposed action. */
  respondAction(id: string, approved: boolean): Promise<void>;

  /** Notch mode; `height` sizes the open bar to its content. */
  setIslandMode(mode: NotchMode, height?: number): Promise<IslandLayout>;
  onIslandLayout(cb: (l: IslandLayout) => void): Unsubscribe;
  /** Close the bar back to the sliver (or hide it when tucked). */
  hideIsland(): Promise<void>;
  setTucked(tucked: boolean): Promise<void>;
  onTucked(cb: (tucked: boolean) => void): Unsubscribe;
  /** The island lost focus (clicked elsewhere). */
  onBlur(cb: () => void): Unsubscribe;
  /** Warm up the connection to Gemini when the bar opens. */
  prewarm(): void;
  /** Hold-to-talk chords from the keyboard hook. */
  onHold(cb: (e: HoldEvent) => void): Unsubscribe;
  openSettings(tab?: string): Promise<void>;
  onSettingsTab(cb: (tab: string) => void): Unsubscribe;
  onFocusPrompt(cb: () => void): Unsubscribe;
  onShortcut(cb: (action: ShortcutAction) => void): Unsubscribe;
  quit(): Promise<void>;

  /** Select to Ask. */
  startCapture(): Promise<void>;
  overlayMonitor(): number;
  capturePreview(monitor: number): Promise<string>;
  captureSelect(monitor: number, rect: SelectionRect): Promise<void>;
  captureSelectLasso(
    monitor: number,
    points: Point[],
    viewWidth: number,
    viewHeight: number,
  ): Promise<void>;
  captureCancel(): Promise<void>;
  /** Point at the on-screen element that answers the question. */
  showMe(question: string): Promise<Highlight[]>;
  highlightsGet(): Promise<Highlight[]>;
  onCaptureReady(cb: (c: CaptureReady) => void): Unsubscribe;
  onCaptureError(cb: (e: WispError) => void): Unsubscribe;

  /** Dictation. */
  createRecorder(): AudioRecorder;
  transcribe(rec: Recording): Promise<string>;
  dictationTarget(): Promise<{ available: boolean; title: string | null }>;
  insertText(text: string): Promise<void>;

  /** Reminders, memory, clipboard (local). */
  tasksList(includeCompleted: boolean): Promise<Task[]>;
  taskCreate(task: NewTask): Promise<Task>;
  taskUpdate(id: string, task: NewTask): Promise<Task>;
  taskComplete(id: string, completed: boolean): Promise<Task>;
  taskSnooze(id: string, until: string): Promise<Task>;
  taskDelete(id: string): Promise<boolean>;
  onReminderDue(cb: (t: Task) => void): Unsubscribe;
  memoryList(): Promise<MemoryEntry[]>;
  memoryAdd(content: string, category: MemoryCategory): Promise<MemoryEntry>;
  memoryUpdate(
    id: string,
    content: string,
    category: MemoryCategory,
    enabled: boolean,
  ): Promise<MemoryEntry>;
  memoryDelete(id: string): Promise<boolean>;
  /** Reads clipboard text once. Only call on an explicit user action. */
  clipboardRead(): Promise<string | null>;

  /** Start-menu apps that can be added to the allowlist. */
  installedApps(): Promise<InstalledApp[]>;
  /** Manual check against the project's GitHub releases. */
  checkForUpdates(): Promise<UpdateInfo>;
  /** Tell the host the UI booted (used by the packaged self-test). */
  uiReady(): void;
  onIslandVisibility(cb: (visible: boolean) => void): Unsubscribe;

  historyList(limit?: number): Promise<RequestLog[]>;
  actionsList(limit?: number): Promise<ActionAudit[]>;
  dataStats(): Promise<StorageStats>;
  dataDelete(scope: DeleteScope): Promise<StorageStats>;
  onDataChanged(cb: (scope: string) => void): Unsubscribe;
  diagnosticsReport(): Promise<string>;
  /** Saves to Downloads; returns the file path. */
  diagnosticsSave(): Promise<string>;

  openExternal(url: string): Promise<void>;
  copyText(text: string): Promise<void>;

  /** Dashboard: controls. */
  controlsState(): Promise<ControlsState>;
  /** What's playing (cheap, cached by the host). */
  nowPlaying(): Promise<NowPlaying | null>;
  setVolume(percent: number): Promise<void>;
  setMuted(muted: boolean): Promise<void>;
  mediaCommand(command: MediaCommand): Promise<void>;
  setRadio(kind: "wifi" | "bluetooth", on: boolean): Promise<void>;
  setDarkMode(dark: boolean): Promise<void>;
  /** Dashboard: stats, weather, network. */
  statsSnapshot(): Promise<StatsSnapshot>;
  speedTest(): Promise<SpeedResult>;
  weatherSearch(query: string): Promise<WeatherPlace[]>;
  weatherGet(): Promise<Weather | null>;
  /** Dashboard: clipboard history. */
  clipsList(kind?: "text" | "link"): Promise<ClipEntry[]>;
  clipPin(id: string, pinned: boolean): Promise<boolean>;
  clipDelete(id: string): Promise<boolean>;
  clipsClear(includePinned: boolean): Promise<number>;
  clipCopy(id: string): Promise<void>;
  /** Dashboard: launcher. */
  /** The launcher catalogue; `refresh` re-scans installed apps and games. */
  launcherList(refresh?: boolean): Promise<Catalogue>;
  launchItem(item: LaunchItem): Promise<void>;
  /** Dashboard: power (the UI confirms first). */
  powerAction(action: PowerAction): Promise<void>;
  /** Dashboard: wallpaper. */
  wallpaperInfo(): Promise<WallpaperInfo>;
  wallpaperThumb(path: string): Promise<string>;
  wallpaperPick(): Promise<string | null>;
  wallpaperSet(path: string): Promise<void>;
  /** Agents. */
  agentStart(kind: AgentKind): Promise<AgentJob>;
  agentCancel(id: string): Promise<void>;
  agentsList(): Promise<AgentJob[]>;
  agentDelete(id: string): Promise<boolean>;
  agentOpen(id: string): Promise<void>;
  /** Process name of the app Wisp was summoned from (local only). */
  foregroundApp(): Promise<string | null>;
}

let current: Host | null = null;

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function initHost(): Promise<Host> {
  if (current) return current;
  if (isTauri()) {
    const { createTauriHost } = await import("./tauriHost");
    current = createTauriHost();
  } else {
    const { createMockHost } = await import("./mockHost");
    current = createMockHost();
  }
  return current;
}

export function host(): Host {
  if (!current) throw new Error("Host not initialised");
  return current;
}

/** Test hook. */
export function setHost(h: Host | null): void {
  current = h;
}
