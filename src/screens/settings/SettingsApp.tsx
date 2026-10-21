import { useEffect, useState, type ComponentType } from "react";
import { Orb } from "../../components/Orb";
import { ToastHost } from "../../components/Toast";
import { host } from "../../services/host";
import type { SettingsSnapshot } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Onboarding } from "./Onboarding";
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
  { id: "model", label: "Model", component: ModelTab },
  { id: "shortcuts", label: "Shortcuts", component: ShortcutsTab },
  { id: "privacy", label: "Privacy", component: PrivacyTab },
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
          <Orb state="idle" size={28} decorative />
          <span>Wisp</span>
        </div>
        <ul role="tablist" aria-orientation="vertical">
          {tabs.map((t) => (
            <li key={t.id}>
              <button
                role="tab"
                id={`tab-${t.id}`}
                aria-selected={!onboarding && t.id === active.id}
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
