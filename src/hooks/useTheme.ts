import { useEffect } from "react";
import type { MotionPreference, ThemePreference } from "../services/types";

function media(q: string): MediaQueryList | null {
  return typeof window !== "undefined" && window.matchMedia ? window.matchMedia(q) : null;
}

export function resolveTheme(pref: ThemePreference, systemDark: boolean): "dark" | "light" {
  if (pref === "system") return systemDark ? "dark" : "light";
  return pref;
}

export function resolveMotion(pref: MotionPreference, systemReduced: boolean): "reduced" | "full" {
  if (pref === "system") return systemReduced ? "reduced" : "full";
  return pref;
}

/** Apply theme + motion preference to <html>, following Windows when set to System. */
export function useThemeEffect(theme: ThemePreference, motion: MotionPreference): void {
  useEffect(() => {
    const dark = media("(prefers-color-scheme: dark)");
    const reduced = media("(prefers-reduced-motion: reduce)");
    const apply = () => {
      const root = document.documentElement;
      root.dataset.theme = resolveTheme(theme, dark?.matches ?? true);
      root.dataset.motion = resolveMotion(motion, reduced?.matches ?? false);
    };
    apply();
    dark?.addEventListener("change", apply);
    reduced?.addEventListener("change", apply);
    return () => {
      dark?.removeEventListener("change", apply);
      reduced?.removeEventListener("change", apply);
    };
  }, [theme, motion]);
}
