import { Suspense, useEffect } from "react";
import { Icon, type IconName } from "../../components/Icon";
import { DASHBOARD_TABS, type DashboardTab } from "../../lib/commands";
import { useNotch } from "../../state/notch";
import { useSettings } from "../../state/settings";
import { preloadAllTabs, TABS } from "./load";

const ICONS: Record<DashboardTab, IconName> = {
  today: "today",
  controls: "sliders",
  stats: "cpu",
  clipboard: "clipboard",
  apps: "grid",
  agents: "bot",
  wallpaper: "image",
  power: "power",
};

/** The tabs the user chose to show, in their order (all of them if unset). */
export function visibleTabs(chosen: readonly string[] | undefined) {
  if (!chosen?.length) return DASHBOARD_TABS;
  const tabs = chosen
    .map((id) => DASHBOARD_TABS.find((t) => t.id === id))
    .filter((t): t is (typeof DASHBOARD_TABS)[number] => !!t);
  return tabs.length ? tabs : DASHBOARD_TABS;
}

export default function Dashboard() {
  const chosen = useSettings((s) => s.snapshot?.settings.dashboardTabs);
  const tabs = visibleTabs(chosen);
  const current = useNotch((n) => n.tab);
  const setTab = useNotch((n) => n.setTab);
  // A hidden tab falls back to the first one shown.
  const tab = tabs.some((t) => t.id === current) ? current : tabs[0]!.id;
  const Panel = TABS[tab];
  useEffect(() => {
    const t = setTimeout(preloadAllTabs, 300);
    return () => clearTimeout(t);
  }, []);
  const move = (dir: 1 | -1) => {
    const i = tabs.findIndex((t) => t.id === tab);
    const next = tabs[(i + dir + tabs.length) % tabs.length]!;
    setTab(next.id);
    document.getElementById(`dash-tab-${next.id}`)?.focus();
  };
  return (
    <div className="dash">
      <div
        className="dash-tabs"
        role="tablist"
        aria-label="Dashboard"
        onKeyDown={(e) => {
          if (e.key === "ArrowRight") move(1);
          if (e.key === "ArrowLeft") move(-1);
        }}
      >
        {tabs.map((t) => (
          <button
            key={t.id}
            id={`dash-tab-${t.id}`}
            role="tab"
            aria-selected={tab === t.id}
            aria-controls="dash-panel"
            tabIndex={tab === t.id ? 0 : -1}
            className={`dash-tab ${tab === t.id ? "active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            <Icon name={ICONS[t.id]} size={15} />
            <span>{t.label}</span>
          </button>
        ))}
      </div>
      <div
        className="dash-panel"
        id="dash-panel"
        role="tabpanel"
        aria-labelledby={`dash-tab-${tab}`}
      >
        <Suspense fallback={<div className="dash-loading" aria-busy="true" />}>
          <Panel />
        </Suspense>
      </div>
    </div>
  );
}
