import { lazy, type ComponentType } from "react";

/**
 * Like React.lazy, but `preload()` fetches the module ahead of time and,
 * once it has loaded, the component renders synchronously (no Suspense
 * fallback, no layout jump).
 */
export function lazyWithPreload<P extends object>(
  loader: () => Promise<{ default: ComponentType<P> }>,
): ComponentType<P> & { preload(): Promise<unknown> } {
  let loaded: ComponentType<P> | null = null;
  let pending: Promise<{ default: ComponentType<P> }> | null = null;
  const load = () =>
    (pending ??= loader().then((m) => {
      loaded = m.default;
      return m;
    }));
  const Lazy = lazy(load);
  const Component = (props: P) => {
    const Loaded = loaded;
    return Loaded ? <Loaded {...props} /> : <Lazy {...props} />;
  };
  Component.preload = load;
  return Component;
}

/** Run `fn` when the browser is idle (or soon, where idle callbacks aren't available). */
export function whenIdle(fn: () => void, timeout = 1500): void {
  const w = globalThis as unknown as {
    requestIdleCallback?: (cb: () => void, o?: { timeout: number }) => number;
  };
  if (w.requestIdleCallback) w.requestIdleCallback(fn, { timeout });
  else setTimeout(fn, 200);
}
