import { create } from "zustand";

/** Capture/playback activity. Kept separate so it can never be confused with idle. */
interface ActivityState {
  listening: boolean;
  speaking: boolean;
  setListening(v: boolean): void;
  setSpeaking(v: boolean): void;
}

export const useActivity = create<ActivityState>((set) => ({
  listening: false,
  speaking: false,
  setListening: (listening) => set({ listening }),
  setSpeaking: (speaking) => set({ speaking }),
}));
