// The single boundary between the UI and the desktop host.
// `tauriHost` talks to Rust; `mockHost` lets the whole UI run in a browser.

import type { WispError } from "./errors";
import type { AudioRecorder, Recording } from "./recorder";
import type {
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
} from "./types";

export type Unsubscribe = () => void;

export type WindowKind = "island" | "settings" | "overlay";

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

  setIslandExpanded(expanded: boolean): Promise<IslandLayout>;
  onIslandLayout(cb: (l: IslandLayout) => void): Unsubscribe;
  hideIsland(): Promise<void>;
  startDragging(): Promise<void>;
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
  captureCancel(): Promise<void>;
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
