import { useEffect, useState, type ComponentType } from "react";
import { WispMark } from "../../components/WispMark";
import { ToastHost } from "../../components/Toast";
import { host } from "../../services/host";
import type { SettingsSnapshot } from "../../services/types";
import { useSettings } from "../../state/settings";
import { DataTab, HistoryTab } from "./DataTabs";
import { ActionsTab, MemoryTab, RemindersTab } from "./LocalTabs";
import { Onboarding } from "./Onboarding";
import { FocusTab } from "./FocusTab";
import { NotchTab } from "./NotchTab";
import { SearchTab } from "./SearchTab";
import { VoiceTab } from "./VoiceTab";
import { AboutTab, GeneralTab, ModelTab, PrivacyTab, ShortcutsTab } from "./tabs";

export interface TabDef {
  id: string;
  label: string;
  component: ComponentType;
}

/** Later milestones append tabs here (History, Memory, Reminders, …). */
export const EXTRA_TABS: TabDef[] = [];

const BASE_TABS: TabDef[] = [
  { id: "general", label: "General", component: GeneralTab },
  { id: "notch", label: "Notch & personality", component: NotchTab },
  { id: "model", label: "Model", component: ModelTab },
  { id: "voice", label: "Voice & dictation", component: VoiceTab },
  { id: "focus", label: "Games & focus", component: FocusTab },
  { id: "shortcuts", label: "Shortcuts", component: ShortcutsTab },
  { id: "search", label: "Search", component: SearchTab },
  { id: "actions", label: "Actions", component: ActionsTab },
  { id: "reminders", label: "Reminders", component: RemindersTab },
  { id: "memory", label: "Memory", component: MemoryTab },
  { id: "privacy", label: "Privacy", component: PrivacyTab },
  { id: "history", label: "History", component: HistoryTab },
  { id: "data", label: "Data", component: DataTab },
];

function needsSetup(s: SettingsSnapshot | null): boolean {
  return s ? !s.settings.onboardingComplete : false;
}

export function SettingsApp() {
  const snapshot = useSettings((s) => s.snapshot);
  const tabs: TabDef[] = [
    ...BASE_TABS,
    ...EXTRA_TABS,
    { id: "about", label: "About", component: AboutTab },
  ];
  const initial = host().initialTab();
  const [tab, setTab] = useState<string>(() =>
    initial && initial !== "welcome" ? initial : "general",
  );
  const [onboarding, setOnboarding] = useState(
    () => initial === "welcome" || needsSetup(useSettings.getState().snapshot),
  );
  const needsOnboarding = needsSetup(snapshot);

  useEffect(
    () =>
      host().onSettingsTab((t) => {
        if (t === "welcome") setOnboarding(true);
        else {
          setOnboarding(false);
          setTab(t);
        }
      }),
    [],
  );
  // Open onboarding whenever settings arrive saying setup isn't finished.
  useEffect(
    () =>
      useSettings.subscribe((st) => {
        if (st.snapshot && !st.snapshot.settings.onboardingComplete) setOnboarding(true);
      }),
    [],
  );
  const active = tabs.find((t) => t.id === tab) ?? tabs[0]!;
  const Active = active.component;

  return (
    <div className="settings">
      <nav className="sidebar" aria-label="Settings sections">
        <div className="brand">
          <WispMark size={26} />
          <span>Wisp</span>
        </div>
        <ul
          role="tablist"
          aria-orientation="vertical"
          onKeyDown={(e) => {
            if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
            const btns = [
              ...e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
            ];
            const i = btns.indexOf(document.activeElement as HTMLButtonElement);
            const next =
              e.key === "Home"
                ? 0
                : e.key === "End"
                  ? btns.length - 1
                  : (i + (e.key === "ArrowDown" ? 1 : -1) + btns.length) % btns.length;
            e.preventDefault();
            btns[next]?.focus();
            btns[next]?.click();
          }}
        >
          {tabs.map((t) => (
            <li key={t.id}>
              <button
                role="tab"
                id={`tab-${t.id}`}
                aria-selected={!onboarding && t.id === active.id}
                tabIndex={t.id === active.id ? 0 : -1}
                aria-controls="settings-panel"
                className={!onboarding && t.id === active.id ? "active" : ""}
                onClick={() => {
                  setOnboarding(false);
                  setTab(t.id);
                }}
                disabled={needsOnboarding}
              >
                {t.label}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <main
        id="settings-panel"
        role="tabpanel"
        aria-labelledby={onboarding ? undefined : `tab-${active.id}`}
      >
        {onboarding ? <Onboarding onFinish={() => setOnboarding(false)} /> : <Active />}
      </main>
      <ToastHost />
    </div>
  );
}
