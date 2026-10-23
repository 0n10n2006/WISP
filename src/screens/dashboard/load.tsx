// Code-split dashboard tabs that can be fetched ahead of time.

import type { ComponentType } from "react";
import type { DashboardTab } from "../../lib/commands";
import { lazyWithPreload } from "../../lib/preload";

type Tab = ComponentType & { preload(): Promise<unknown> };

export const TABS: Record<DashboardTab, Tab> = {
  today: lazyWithPreload(() => import("./TodayTab")),
  controls: lazyWithPreload(() => import("./ControlsTab")),
  stats: lazyWithPreload(() => import("./StatsTab")),
  clipboard: lazyWithPreload(() => import("./ClipboardTab")),
  apps: lazyWithPreload(() => import("./AppsTab")),
  agents: lazyWithPreload(() => import("./AgentsTab")),
  wallpaper: lazyWithPreload(() => import("./WallpaperTab")),
  power: lazyWithPreload(() => import("./PowerTab")),
};

export const Dashboard = lazyWithPreload(() => import("./Dashboard"));

/** Fetch the dashboard and the tab it will open on. */
export function preloadDashboard(tab: DashboardTab): void {
  void Dashboard.preload();
  // A tab saved by an older version may not exist any more.
  void TABS[tab]?.preload();
}

/** Fetch every tab (after the bar has opened once). */
export function preloadAllTabs(): void {
  for (const t of Object.values(TABS)) void t.preload();
}
