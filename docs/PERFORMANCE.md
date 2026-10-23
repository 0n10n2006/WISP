# Wisp performance

Wisp is meant to be invisible until you need it and instant when you do. This
page records how that is measured and the numbers from the 0.2.1 tuning pass.

## How it's measured

- **UI harness:** `scripts/perf/notch-perf.mjs`.
  - Runs the production build (`vite preview`) in Chromium against the mock host, several times, and reports medians.
  - Measures: JS loaded at start, idle cost while only the sliver is showing (CPU time, layouts and style recalcs over 5 s), time from click to a stable open bar, window resizes per open, the same for a warm re-open, long tasks (over 50 ms), and keystroke-to-results latency.
- **Visual review:** `scripts/perf/notch-shots.mjs` captures every notch state, dashboard tab and Settings, in dark and light Windows themes.
- **Desktop start-up:** `wisp --self-test` records start-up phases, including `sliver-shown`. CI runs it on the packaged Windows build, and on Linux headless.

```bash
npm run build && npx vite preview --port 4173 &
PW_CHROMIUM_PATH=/path/to/chromium node scripts/perf/notch-perf.mjs 7
node scripts/perf/notch-shots.mjs ./shots dark
```

## Results (median of 5–7 runs, same machine)

| Metric                                    | v0.2.0 | v0.2.1  | v0.2.2  | v0.2.5  |
| ----------------------------------------- | ------ | ------- | ------- | ------- |
| JS loaded at start (transferred)          | 104 KB | 36 KB   | 37 KB   | 42 KB   |
| Island bundle (raw)                       | 274 KB | 29 KB   | 30 KB   | 31 KB   |
| Idle cost, 5 s with only the sliver       | ~0 ms  | ~0 ms   | ~0 ms   | ~0 ms   |
| Click → bar open and stable (cold)        | 429 ms | ~145 ms | ~147 ms | ~120 ms |
| Window resizes per open (cold)            | 4      | 3       | 3       | 3       |
| Re-open → bar shown (warm)                | —      | ~19 ms  | ~25 ms  | ~21 ms  |
| Window resizes per re-open                | —      | 1       | 1       | 1       |
| Long tasks while opening                  | 0–2    | 0       | 0       | 0       |
| Keystroke → results                       | 54 ms  | 41 ms   | 46 ms   | 37 ms   |
| Warm start → sliver on screen (self-test) | 680 ms | ~350 ms | ~355 ms | ~355 ms |

Cold-open time includes the hover peek that the harness triggers on its way to the click. On a direct shortcut press, the bar renders in about 30 ms and settles in about 80 ms.

## What changed

- **Preact instead of React DOM.**
  - Same components and hooks through `preact/compat`, at a fraction of the size.
  - Every Wisp window (notch, settings, overlays) parses and runs less JavaScript.
  - Tests render through Preact too.
- **Smaller first load:**
  - Only the sliver and peek ship in the first chunk.
  - The bar, dashboard and tabs load while the PC is idle. They render with no loading flash, because `lazyWithPreload` renders synchronously once a module is ready.
- **One resize per open:**
  - The bar opens at its remembered height and reports a new height only after it has been stable for two frames, in 4 px steps.
  - The host skips moves and resizes that don't change anything, and keeps the bar on the monitor it opened on.
- **Cheaper Windows calls:**
  - Volume, media and radios run on one long-lived COM worker thread that keeps its objects.
  - "Now playing" for the peek is a dedicated, cached call instead of the full controls query.
- **Launcher cached:** Start menu and Steam are scanned once in the background after start-up; the Apps tab has a rescan button.
- **Lighter stats:** disks are kept and refreshed every tenth poll, processes every other poll.
- **Faster start-up:**
  - The notch is shown before global shortcuts are registered.
  - History pruning and the launcher scan run 1.5 s after start-up.
  - The taskbar theme check runs every 5 s instead of every second.
- **Animations:**
  - Only `transform` and `opacity` are animated.
  - The idle flame no longer flickers forever.
  - The sliver glow is an opacity change on its own layer.
  - Everything pauses while Wisp is hidden or in reduced motion.
- **Rendering:** finished messages are memoised, so streaming an answer re-renders only that answer.
