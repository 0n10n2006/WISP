import { useEffect } from "react";
import { useThemeEffect } from "./hooks/useTheme";
import { host } from "./services/host";
import { Island } from "./screens/Island";
import { SelectOverlay } from "./screens/SelectOverlay";
import { SettingsApp } from "./screens/settings/SettingsApp";
import { subscribeSettings, useSettings } from "./state/settings";
import { DEFAULT_SETTINGS } from "./services/types";

export function App() {
  const snapshot = useSettings((s) => s.snapshot);
  const load = useSettings((s) => s.load);
  const kind = host().windowKind();

  useEffect(() => {
    void load();
    return subscribeSettings();
  }, [load]);

  const settings = snapshot?.settings ?? DEFAULT_SETTINGS;
  useThemeEffect(settings.theme, settings.motion, settings.islandScale);

  useEffect(() => {
    document.documentElement.dataset.window = kind;
  }, [kind]);

  // Pause decorative animation whenever Wisp isn't visible (PRD §22.1).
  useEffect(() => {
    const root = document.documentElement;
    const sync = () => {
      if (document.hidden) root.dataset.hidden = "true";
      else delete root.dataset.hidden;
    };
    document.addEventListener("visibilitychange", sync);
    const off = host().onIslandVisibility((visible) => {
      if (visible && !document.hidden) delete root.dataset.hidden;
      else root.dataset.hidden = "true";
    });
    return () => {
      document.removeEventListener("visibilitychange", sync);
      off();
    };
  }, []);

  if (kind === "island") return <Island />;
  if (kind === "overlay") return <SelectOverlay />;
  return <SettingsApp />;
}
