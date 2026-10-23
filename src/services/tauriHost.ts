import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toWispError } from "./errors";
import type { AskHandle, Host, Unsubscribe, WindowKind } from "./host";
import { createMediaRecorder } from "./recorder";
import type { WispError } from "./errors";
import type {
  ActionEvent,
  Task,
  AskResult,
  CaptureReady,
  IslandLayout,
  SettingsSnapshot,
  ShortcutAction,
  HoldEvent,
} from "./types";

/** Note a problem in the host's local error log (never throws). */
function report(message: string): void {
  void invoke("report_ui_error", { message: message.slice(0, 2000) }).catch(() => {});
}

/** Errors that are part of normal use and not worth logging. */
const EXPECTED = new Set([
  "cancelled",
  "invalid",
  "missing_key",
  "rate_limited",
  "quota_exhausted",
  "unsupported",
  "permission_denied",
]);

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    const err = toWispError(e);
    if (!EXPECTED.has(err.code)) report(`${cmd} failed: ${err.code}: ${err.message}`);
    throw err;
  }
}

function subscribe<T>(event: string, cb: (payload: T) => void): Unsubscribe {
  let un: UnlistenFn | null = null;
  let cancelled = false;
  void listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (cancelled) fn();
    else un = fn;
  });
  return () => {
    cancelled = true;
    un?.();
  };
}

export function createTauriHost(): Host {
  const label = getCurrentWindow().label;
  const kind: WindowKind =
    label === "island"
      ? "island"
      : label.startsWith("overlay")
        ? "overlay"
        : label === "highlight"
          ? "highlight"
          : "settings";

  return {
    kind: "tauri",
    windowKind: () => kind,
    initialTab: () => new URLSearchParams(window.location.search).get("tab"),

    getSettings: () => call<SettingsSnapshot>("get_settings"),
    updateSettings: (patch) => call<SettingsSnapshot>("update_settings", { patch }),
    resetShortcuts: () => call<SettingsSnapshot>("reset_shortcuts"),
    resetSettings: () => call<SettingsSnapshot>("reset_settings"),
    validateShortcut: (accelerator) => call<string>("validate_shortcut", { accelerator }),
    onSettings: (cb) => subscribe<SettingsSnapshot>("wisp://settings", cb),

    setApiKey: (key) => call<SettingsSnapshot>("set_api_key", { key }),
    removeApiKey: () => call<SettingsSnapshot>("remove_api_key"),
    testConnection: (key, model) =>
      call("test_connection", { key: key ?? null, model: model ?? null }),
    listModels: () => call("list_models"),

    ask(req, onDelta, onAction): AskHandle {
      const channel = new Channel<{ type: "delta"; text: string } | ActionEvent>();
      channel.onmessage = (m) => {
        if (m.type === "delta") onDelta(m.text);
        else onAction?.(m);
      };
      const result = call<AskResult>("ask", { request: req, onEvent: channel });
      return {
        result,
        cancel: () => void call("ask_cancel", { requestId: req.requestId }),
      };
    },

    setIslandMode: (mode, height) =>
      call<IslandLayout>("set_island_mode", { mode, height: height ?? null }),
    onIslandLayout: (cb) => subscribe<IslandLayout>("wisp://layout", cb),
    hideIsland: () => call("hide_island"),
    setTucked: (tucked) => call("set_tucked", { tucked }),
    onTucked: (cb) => subscribe<boolean>("wisp://tucked", cb),
    onBlur: (cb) => subscribe<null>("wisp://blur", () => cb()),
    prewarm: () => void invoke("prewarm").catch(() => {}),
    onHold: (cb) => subscribe<HoldEvent>("wisp://hold", cb),
    openSettings: (tab) => call("open_settings", { tab: tab ?? null }),
    onSettingsTab: (cb) => subscribe<string>("wisp://settings-tab", cb),
    onFocusPrompt: (cb) => subscribe<null>("wisp://focus-prompt", () => cb()),
    onShortcut: (cb) => subscribe<ShortcutAction>("wisp://shortcut", cb),
    quit: () => call("quit_app"),

    startCapture: () => call("capture_start"),
    overlayMonitor: () => Number(new URLSearchParams(window.location.search).get("monitor") ?? 0),
    capturePreview: async (monitor) => {
      // Raw JPEG bytes (no base64): an object URL the overlay can paint.
      const bytes = await call<ArrayBuffer>("capture_preview", { monitor });
      return URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
    },
    captureOverlayReady: () => void call("capture_overlay_ready").catch(() => {}),
    onCaptureSession: (cb) => subscribe<number>("wisp://capture-session", () => cb()),
    captureSelect: (monitor, rect) => call("capture_select", { monitor, rect }),
    captureSelectLasso: (monitor, points, viewWidth, viewHeight) =>
      call("capture_select_lasso", { monitor, points, viewWidth, viewHeight }),
    captureCancel: () => call("capture_cancel"),
    showMe: (question) => call("show_me", { question }),
    highlightsGet: () => call("highlights_get"),
    onCaptureReady: (cb) => subscribe<CaptureReady>("wisp://capture-ready", cb),
    onCaptureError: (cb) => subscribe<WispError>("wisp://capture-error", cb),

    createRecorder: () => createMediaRecorder(),
    transcribe: (rec) => call("transcribe", { audioBase64: rec.base64, mimeType: rec.mimeType }),
    dictationTarget: () => call("dictation_target"),
    insertText: (text) => call("insert_text", { text }),

    respondAction: (id, approved) => call("action_respond", { id, approved }),
    tasksList: (includeCompleted) => call("tasks_list", { includeCompleted }),
    taskCreate: (task) => call("task_create", { task }),
    taskUpdate: (id, task) => call("task_update", { id, task }),
    taskComplete: (id, completed) => call("task_complete", { id, completed }),
    taskSnooze: (id, until) => call("task_snooze", { id, until }),
    taskDelete: (id) => call("task_delete", { id }),
    onReminderDue: (cb) => subscribe<Task>("wisp://reminder-due", cb),
    memoryList: () => call("memory_list"),
    memoryAdd: (content, category) => call("memory_add", { content, category }),
    memoryUpdate: (id, content, category, enabled) =>
      call("memory_update", { id, content, category, enabled }),
    memoryDelete: (id) => call("memory_delete", { id }),
    clipboardRead: async () => (await call<{ text: string | null }>("clipboard_read")).text,

    installedApps: () => call("installed_apps"),
    checkForUpdates: () => call("check_for_updates"),
    uiReady: () => void invoke("ui_ready").catch((e) => report(`ui_ready failed: ${String(e)}`)),
    onIslandVisibility: (cb) => subscribe<boolean>("wisp://visibility", cb),

    historyList: (limit) => call("history_list", { limit: limit ?? null }),
    actionsList: (limit) => call("actions_list", { limit: limit ?? null }),
    dataStats: () => call("data_stats"),
    dataDelete: (scope) => call("data_delete", { scope }),
    onDataChanged: (cb) => subscribe<string>("wisp://data-changed", cb),
    diagnosticsReport: () => call("diagnostics_report"),
    diagnosticsSave: () => call("diagnostics_save"),

    openExternal: (url) => call("open_external", { url }),
    copyText: (text) => call("copy_text", { text }),

    controlsState: () => call("controls_state"),
    nowPlaying: () => call("now_playing"),
    setVolume: (percent) => call("set_volume", { percent: Math.round(percent) }),
    setMuted: (muted) => call("set_muted", { muted }),
    mediaCommand: (command) => call("media_command", { command }),
    setRadio: (kind, on) => call("set_radio", { kind, on }),
    setDarkMode: (dark) => call("set_dark_mode", { dark }),
    statsSnapshot: () => call("stats_snapshot"),
    speedTest: () => call("speed_test"),
    weatherSearch: (query) => call("weather_search", { query }),
    weatherGet: () => call("weather_get"),
    clipsList: (kind) => call("clips_list", { kind: kind ?? null }),
    clipPin: (id, pinned) => call("clip_pin", { id, pinned }),
    clipDelete: (id) => call("clip_delete", { id }),
    clipsClear: (includePinned) => call("clips_clear", { includePinned }),
    clipCopy: (id) => call("clip_copy", { id }),
    launcherList: (refresh) => call("launcher_list", { refresh: refresh ?? false }),
    launchItem: (item) => call("launch_item", { item }),
    powerAction: (action) => call("power_action", { action }),
    wallpaperInfo: () => call("wallpaper_info"),
    wallpaperThumb: (path) => call("wallpaper_thumb", { path }),
    wallpaperPick: () => call("wallpaper_pick"),
    wallpaperSet: (path) => call("wallpaper_set", { path }),
    agentStart: (kind) => call("agent_start", { kind }),
    agentCancel: (id) => call("agent_cancel", { id }),
    agentsList: () => call("agents_list"),
    agentDelete: (id) => call("agent_delete", { id }),
    agentOpen: (id) => call("agent_open", { id }),
    foregroundApp: () => call("foreground_app"),
    openWindows: () => call("open_windows"),
    switchWindow: (id) => call("switch_window", { id }),
    screenStatus: () => call("screen_status"),

    reportError: report,
    pauseShortcuts: (paused) => void call("pause_shortcuts", { paused }).catch(() => {}),
    onSelfTestRun: (cb) => subscribe<string>("wisp://selftest-run", cb),
    selfTestProbe: () => call("selftest_probe"),
    selfTestAction: (action) => call("selftest_action", { action }),
    selfTestReport: (section, steps) => call("selftest_report", { section, steps }),
  };
}
