import { create } from "zustand";

/** Microphone activity. Kept separate so it can never be confused with idle. */
interface ActivityState {
  listening: boolean;
  setListening(v: boolean): void;
}

export const useActivity = create<ActivityState>((set) => ({
  listening: false,
  setListening: (listening) => set({ listening }),
}));
