import { create } from "zustand";

export interface Toast {
  id: number;
  text: string;
  tone: "neutral" | "success" | "warning";
}

interface ToastState {
  toast: Toast | null;
  show(text: string, tone?: Toast["tone"], ms?: number): void;
  hide(): void;
}

let timer: ReturnType<typeof setTimeout> | null = null;
let n = 0;

export const useToast = create<ToastState>((set) => ({
  toast: null,
  show(text, tone = "neutral", ms = 1800) {
    if (timer) clearTimeout(timer);
    n += 1;
    set({ toast: { id: n, text, tone } });
    timer = setTimeout(() => set({ toast: null }), ms);
  },
  hide() {
    if (timer) clearTimeout(timer);
    set({ toast: null });
  },
}));

export const toast = (text: string, tone?: Toast["tone"], ms?: number) =>
  useToast.getState().show(text, tone, ms);
