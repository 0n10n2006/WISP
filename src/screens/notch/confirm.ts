import { create } from "zustand";

/** An inline "are you sure?" strip in the bar. */
interface ConfirmState {
  pending: { text: string; label: string; run: () => Promise<void> } | null;
  ask(text: string, label: string, run: () => Promise<void>): void;
  clear(): void;
}

export const useConfirm = create<ConfirmState>((set) => ({
  pending: null,
  ask: (text, label, run) => set({ pending: { text, label, run } }),
  clear: () => set({ pending: null }),
}));
