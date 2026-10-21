import { lazy, Suspense, useEffect } from "react";
import { useThemeEffect } from "./hooks/useTheme";
import { Notch } from "./screens/notch/Notch";
import { host } from "./services/host";
import { DEFAULT_SETTINGS } from "./services/types";
import { subscribeSettings, useSettings } from "./state/settings";

const SelectOverlay = lazy(() =>
  import("./screens/SelectOverlay").then((m) => ({ default: m.SelectOverlay })),
);
const Highlight = lazy(() => import("./screens/Highlight"));
const SettingsApp = lazy(() =>
  import("./screens/settings/SettingsApp").then((m) => ({ default: m.SettingsApp })),
);

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
  const accent = settings.accent;
  useEffect(() => {
    document.documentElement.dataset.accent = accent;
  }, [accent]);

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

  if (kind === "island") return <Notch />;
  return (
    <Suspense fallback={null}>
      {kind === "overlay" ? (
        <SelectOverlay />
      ) : kind === "highlight" ? (
        <Highlight />
      ) : (
        <SettingsApp />
      )}
    </Suspense>
  );
}
