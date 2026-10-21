import { create } from "zustand";
import { host } from "../services/host";
import { toWispError, type WispError } from "../services/errors";
import type { DeepPartial, Settings, SettingsSnapshot } from "../services/types";

interface SettingsState {
  snapshot: SettingsSnapshot | null;
  loadError: WispError | null;
  load(): Promise<void>;
  /** Returns an error instead of throwing so forms can show it inline. */
  update(patch: DeepPartial<Settings>): Promise<WispError | null>;
  apply(snapshot: SettingsSnapshot): void;
}

export const useSettings = create<SettingsState>((set) => ({
  snapshot: null,
  loadError: null,
  async load() {
    try {
      set({ snapshot: await host().getSettings(), loadError: null });
    } catch (e) {
      set({ loadError: toWispError(e) });
    }
  },
  async update(patch) {
    try {
      set({ snapshot: await host().updateSettings(patch) });
      return null;
    } catch (e) {
      return toWispError(e);
    }
  },
  apply(snapshot) {
    set({ snapshot });
  },
}));

export function subscribeSettings(): () => void {
  return host().onSettings((s) => useSettings.getState().apply(s));
}
