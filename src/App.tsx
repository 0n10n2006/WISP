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
  const accentColor = settings.accentColor;
  useEffect(() => {
    const root = document.documentElement;
    root.dataset.accent = accent;
    root.style.setProperty("--accent-custom", accentColor);
  }, [accent, accentColor]);

  // `--self-test` asks each window to run its part of the UI scenario.
  useEffect(
    () =>
      host().onSelfTestRun(
        (section) => void import("./lib/selftest").then((m) => m.runSelfTest(section)),
      ),
    [],
  );

  // Pause decorative animation whenever Wisp isn't visible (PRD §22.1).
  useEffect(() => {
    const root = document.documentElement;
    const sync = () => {
      if (document.hidden) root.dataset.hidden = "true";
      else delete root.dataset.hidden;
    };
    document.addEventListener("visibilitychange", sync);
    // Island visibility events reach every window; only the island pauses.
    const off =
      kind === "island"
        ? host().onIslandVisibility((visible) => {
            if (visible && !document.hidden) delete root.dataset.hidden;
            else root.dataset.hidden = "true";
          })
        : () => {};
    return () => {
      document.removeEventListener("visibilitychange", sync);
      off();
    };
  }, [kind]);

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
