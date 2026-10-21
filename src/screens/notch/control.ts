// Opening and closing the bar. Kept tiny so the always-loaded shell stays small.

import { host } from "../../services/host";
import { useNotch } from "../../state/notch";
import { useConfirm } from "./confirm";

const HEIGHT_KEY = "wisp.notch.barHeight";

export function lastHeight(): number | undefined {
  try {
    const v = Number(localStorage.getItem(HEIGHT_KEY));
    return Number.isFinite(v) && v > 0 ? v : undefined;
  } catch {
    return undefined;
  }
}

/** Remember the bar's height so the next open starts at the right size. */
export function rememberBarHeight(h: number): void {
  try {
    localStorage.setItem(HEIGHT_KEY, String(h));
  } catch {
    /* ignore */
  }
}

export function openBar(): void {
  if (useNotch.getState().mode !== "open") {
    useNotch.getState().setMode("open");
    void host().setIslandMode("open", lastHeight());
  }
  host().prewarm();
}

export function closeBar(): void {
  useConfirm.getState().clear();
  void host().hideIsland();
}
