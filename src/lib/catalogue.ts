import { useEffect, useState } from "react";
import { host } from "../services/host";
import type { Catalogue } from "../services/types";

// The launcher catalogue (apps, games, Windows pages). The host scans once
// and caches; the UI keeps one copy per session and refreshes "recent"
// after a launch.

let cache: Catalogue | null = null;
let pending: Promise<Catalogue> | null = null;
const listeners = new Set<(c: Catalogue) => void>();

function load(force = false, rescan = false): Promise<Catalogue> {
  if (!pending || force) {
    pending = host()
      .launcherList(rescan)
      .then((c) => {
        cache = c;
        listeners.forEach((l) => l(c));
        return c;
      })
      .catch(() => cache ?? { items: [], recent: [] });
  }
  return pending;
}

/** Fetch ahead of time (e.g. while idle). */
export function prefetchCatalogue(): void {
  void load();
}

/** Refresh after a launch (recent apps) or a full re-scan. */
export function invalidateCatalogue(rescan = false): void {
  void load(true, rescan);
}

export function useCatalogue(): Catalogue | null {
  const [cat, setCat] = useState<Catalogue | null>(cache);
  useEffect(() => {
    listeners.add(setCat);
    void load().then(setCat);
    return () => {
      listeners.delete(setCat);
    };
  }, []);
  return cat;
}
