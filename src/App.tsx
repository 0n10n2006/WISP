import { useEffect } from "react";
import { useThemeEffect } from "./hooks/useTheme";
import { host } from "./services/host";
import { Island } from "./screens/Island";
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
  useThemeEffect(settings.theme, settings.motion);

  useEffect(() => {
    document.documentElement.dataset.window = kind;
  }, [kind]);

  if (kind === "island") return <Island />;
  return <SettingsApp />;
}
