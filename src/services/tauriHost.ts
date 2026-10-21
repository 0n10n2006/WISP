import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toWispError } from "./errors";
import type { AskHandle, Host, Unsubscribe, WindowKind } from "./host";
import type { WispError } from "./errors";
import type {
  AskResult,
  CaptureReady,
  IslandLayout,
  SettingsSnapshot,
  ShortcutAction,
} from "./types";

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toWispError(e);
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
    label === "island" ? "island" : label.startsWith("overlay") ? "overlay" : "settings";

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

    ask(req, onDelta): AskHandle {
      const channel = new Channel<{ type: "delta"; text: string }>();
      channel.onmessage = (m) => {
        if (m.type === "delta") onDelta(m.text);
      };
      const result = call<AskResult>("ask", { request: req, onEvent: channel });
      return {
        result,
        cancel: () => void call("ask_cancel", { requestId: req.requestId }),
      };
    },

    setIslandExpanded: (expanded) => call<IslandLayout>("set_island_expanded", { expanded }),
    onIslandLayout: (cb) => subscribe<IslandLayout>("wisp://layout", cb),
    hideIsland: () => call("hide_island"),
    startDragging: () => getCurrentWindow().startDragging(),
    openSettings: (tab) => call("open_settings", { tab: tab ?? null }),
    onSettingsTab: (cb) => subscribe<string>("wisp://settings-tab", cb),
    onFocusPrompt: (cb) => subscribe<null>("wisp://focus-prompt", () => cb()),
    onShortcut: (cb) => subscribe<ShortcutAction>("wisp://shortcut", cb),
    quit: () => call("quit_app"),

    startCapture: () => call("capture_start"),
    overlayMonitor: () => Number(new URLSearchParams(window.location.search).get("monitor") ?? 0),
    capturePreview: (monitor) => call("capture_preview", { monitor }),
    captureSelect: (monitor, rect) => call("capture_select", { monitor, rect }),
    captureCancel: () => call("capture_cancel"),
    onCaptureReady: (cb) => subscribe<CaptureReady>("wisp://capture-ready", cb),
    onCaptureError: (cb) => subscribe<WispError>("wisp://capture-error", cb),

    historyList: (limit) => call("history_list", { limit: limit ?? null }),
    actionsList: (limit) => call("actions_list", { limit: limit ?? null }),
    dataStats: () => call("data_stats"),
    dataDelete: (scope) => call("data_delete", { scope }),
    onDataChanged: (cb) => subscribe<string>("wisp://data-changed", cb),
    diagnosticsReport: () => call("diagnostics_report"),
    diagnosticsSave: () => call("diagnostics_save"),

    openExternal: (url) => call("open_external", { url }),
    copyText: (text) => call("copy_text", { text }),
  };
}
