// Opening and closing the bar. Kept tiny so the always-loaded shell stays small.

import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { NotchMode } from "../../services/types";
import { useNotch } from "../../state/notch";
import { toast } from "../../state/toast";
import { useConfirm } from "./confirm";

/** How long the host may take to resize the notch before we give up. */
const MODE_TIMEOUT_MS = 2500;

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

/**
 * Switch the notch to `mode`: the UI changes at once and the host resizes the
 * window. If the host can't (an error, or no answer), the UI goes back to
 * what is really on screen and says so, instead of silently doing nothing.
 */
export async function changeMode(mode: NotchMode, height?: number): Promise<boolean> {
  const notch = useNotch.getState();
  const before = notch.mode;
  notch.setMode(mode);
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const layout = await Promise.race([
      height === undefined ? host().setIslandMode(mode) : host().setIslandMode(mode, height),
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`The window didn't respond within ${MODE_TIMEOUT_MS} ms`)),
          MODE_TIMEOUT_MS,
        );
      }),
    ]);
    // The host may keep a different mode (e.g. the bar is already open).
    if (layout && layout.mode !== mode) useNotch.getState().setMode(layout.mode);
    return true;
  } catch (e) {
    const err = toWispError(e);
    host().reportError(`set_island_mode(${mode}) failed: ${err.code}: ${err.message}`);
    if (useNotch.getState().mode === mode) useNotch.getState().setMode(before);
    if (mode === "open")
      toast("Wisp couldn't open. Details are in Settings → Data.", "warning", 4000);
    return false;
  } finally {
    clearTimeout(timer);
  }
}

let blurCloseSuspended = false;

/**
 * While on, losing focus doesn't fold the bar away. Only the self-test uses
 * it: headless test displays move focus around unpredictably, and the
 * scripted steps aren't about focus.
 */
export function suspendBlurClose(on: boolean): void {
  blurCloseSuspended = on;
}

export function blurCloseAllowed(): boolean {
  return !blurCloseSuspended;
}

export function openBar(): void {
  if (useNotch.getState().mode !== "open") void changeMode("open", lastHeight());
  host().prewarm();
}

export function closeBar(): void {
  useConfirm.getState().clear();
  // Fold back locally too: the host only reports a layout change when its own
  // mode changed, so the UI must never wait on it to leave "open".
  useNotch.getState().setMode("sliver");
  void host()
    .hideIsland()
    .catch((e: unknown) => host().reportError(`hide_island failed: ${toWispError(e).message}`));
}
