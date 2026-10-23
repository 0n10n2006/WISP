import { create } from "zustand";
import { DASHBOARD_TABS, type DashboardTab } from "../lib/commands";
import { host } from "../services/host";
import type { NotchMode } from "../services/types";
import { toast } from "./toast";

const TAB_KEY = "wisp.notch.tab";
const TIMER_KEY = "wisp.notch.timer";

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: unknown): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage unavailable */
  }
}

/** A saved tab, if it still exists. */
function knownTab(t: string): DashboardTab {
  return DASHBOARD_TABS.find((x) => x.id === t)?.id ?? "today";
}

export interface FocusTimer {
  startedAt: number;
  endsAt: number;
  minutes: number;
  /** Reminder that delivers the "done" notification even if the bar is closed. */
  taskId: string | null;
}

interface NotchState {
  mode: NotchMode;
  tab: DashboardTab;
  query: string;
  /** Ground the next question with Google Search. */
  web: boolean;
  /** Let the model think longer for the next question. */
  think: boolean;
  timer: FocusTimer | null;
  setMode(mode: NotchMode): void;
  setTab(tab: DashboardTab): void;
  setQuery(q: string): void;
  setWeb(v: boolean): void;
  setThink(v: boolean): void;
  startTimer(minutes: number): Promise<void>;
  stopTimer(): Promise<void>;
  /** Called by the ticking clock when the timer has run out. */
  finishTimer(): void;
}

export const useNotch = create<NotchState>((set, get) => ({
  mode: "sliver",
  tab: knownTab(load<string>(TAB_KEY, "today")),
  query: "",
  web: false,
  think: false,
  timer: load<FocusTimer | null>(TIMER_KEY, null),

  setMode: (mode) => set({ mode }),
  setTab(tab) {
    store(TAB_KEY, tab);
    set({ tab });
  },
  setQuery: (query) => set({ query }),
  setWeb: (web) => set({ web }),
  setThink: (think) => set({ think }),

  async startTimer(minutes) {
    await get().stopTimer();
    const startedAt = Date.now();
    const endsAt = startedAt + Math.round(minutes * 60_000);
    let taskId: string | null = null;
    try {
      const t = await host().taskCreate({
        title: "Focus session finished",
        notes: `${Math.round(minutes)} minute focus timer from Wisp`,
        dueAt: new Date(endsAt).toISOString(),
        notificationEnabled: true,
      });
      taskId = t.id;
    } catch {
      /* the notch still shows the countdown */
    }
    const timer = { startedAt, endsAt, minutes, taskId };
    store(TIMER_KEY, timer);
    set({ timer });
  },

  async stopTimer() {
    const t = get().timer;
    store(TIMER_KEY, null);
    set({ timer: null });
    if (t?.taskId && t.endsAt > Date.now())
      await host()
        .taskDelete(t.taskId)
        .catch(() => {});
  },

  finishTimer() {
    if (!get().timer) return;
    store(TIMER_KEY, null);
    set({ timer: null });
    toast("Focus session finished", "success", 4000);
  },
}));

/** mm:ss (or h:mm:ss) left on the timer. */
export function formatCountdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = String(m).padStart(h ? 2 : 1, "0");
  return `${h ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}
