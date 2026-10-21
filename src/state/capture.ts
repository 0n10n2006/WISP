import { create } from "zustand";
import type { CaptureReady } from "../services/types";

/** A screen-region capture waiting for the user to confirm sending it. */
interface CaptureState {
  pending: CaptureReady | null;
  setPending(c: CaptureReady | null): void;
}

export const useCapture = create<CaptureState>((set) => ({
  pending: null,
  setPending: (pending) => set({ pending }),
}));

export const DEFAULT_SCREEN_PROMPT =
  "Describe what's in this part of my screen and explain anything notable.";
