// Notch performance harness: runs the production build in Chromium against
// the mock host and reports load size, open latency, resize churn, long
// tasks and idle cost. Usage:
//   npm run build && npx vite preview --port 4173 &
//   PW_CHROMIUM_PATH=/path/to/chromium node scripts/perf/notch-perf.mjs [runs]
import { chromium } from "@playwright/test";

const URL = process.env.WISP_PERF_URL ?? "http://localhost:4173/";
const RUNS = Number(process.argv[2] ?? 5);

const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
};

async function once(browser) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Network.enable");
  await cdp.send("Performance.enable");
  let jsBytes = 0;
  const scripts = new Set();
  cdp.on("Network.responseReceived", (e) => {
    if (e.type === "Script") scripts.add(e.requestId);
  });
  cdp.on("Network.loadingFinished", (e) => {
    if (scripts.has(e.requestId)) jsBytes += e.encodedDataLength;
  });
  await page.addInitScript(() => {
    localStorage.setItem("wisp.mock.hasKey", "1");
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "dark" }),
    );
    window.__long = [];
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) window.__long.push(e.duration);
    }).observe({ type: "longtask", buffered: true });
  });
  await page.goto(URL);
  await page.waitForSelector(".notch-idle");
  const loadMs = await page.evaluate(() => performance.now());
  const initialJs = jsBytes;

  // Idle cost while only the sliver is visible.
  await page.waitForTimeout(1500);
  const m0 = Object.fromEntries(
    (await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]),
  );
  await page.waitForTimeout(5000);
  const m1 = Object.fromEntries(
    (await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]),
  );
  const idle = {
    taskMs: (m1.TaskDuration - m0.TaskDuration) * 1000,
    layouts: m1.LayoutCount - m0.LayoutCount,
    styles: m1.RecalcStyleCount - m0.RecalcStyleCount,
  };

  // Open the bar and wait until its height is stable for 250 ms.
  const before = await page.evaluate(() => window.wispMock.metrics.modeCalls);
  await page.evaluate(() => {
    window.__long = [];
    window.__t0 = performance.now();
  });
  await page.click(".notch-idle");
  await page.waitForSelector(".dash-panel [class]:not(.dash-loading)", { timeout: 10000 });
  const openMs = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const el = document.querySelector('[data-testid="island-window"]');
        let last = el.getBoundingClientRect().height;
        let stableSince = performance.now();
        const tick = () => {
          const h = el.getBoundingClientRect().height;
          if (h !== last) {
            last = h;
            stableSince = performance.now();
          }
          if (performance.now() - stableSince > 250) resolve(stableSince - window.__t0);
          else requestAnimationFrame(tick);
        };
        tick();
      }),
  );
  const resizes = (await page.evaluate(() => window.wispMock.metrics.modeCalls)) - before;
  const openLong = await page.evaluate(() => window.__long.slice());

  // Close and open again: the common case (chunks loaded, height remembered).
  await page.keyboard.press("Escape");
  await page.waitForSelector(".notch-idle");
  await page.waitForTimeout(300);
  const before2 = await page.evaluate(() => window.wispMock.metrics.modeCalls);
  await page.evaluate(() => {
    window.__t0 = performance.now();
    window.wispMock.focus();
  });
  const reopenMs = await page.evaluate(
    () =>
      new Promise((resolve) => {
        const tick = () => {
          const bar = document.querySelector(".dash-panel [class]:not(.dash-loading)");
          if (bar) resolve(performance.now() - window.__t0);
          else requestAnimationFrame(tick);
        };
        tick();
      }),
  );
  await page.waitForTimeout(300);
  const reopenResizes = (await page.evaluate(() => window.wispMock.metrics.modeCalls)) - before2;

  // Typing latency.
  await page.evaluate(() => (window.__t1 = performance.now()));
  await page.keyboard.type("spo");
  await page.waitForSelector('[role="option"]');
  const typeMs = await page.evaluate(() => performance.now() - window.__t1);
  const allJs = jsBytes;
  await page.close();
  return {
    initialJsKB: initialJs / 1024,
    allJsKB: allJs / 1024,
    loadMs,
    idleTaskMs: idle.taskMs,
    idleLayouts: idle.layouts,
    idleStyles: idle.styles,
    openMs,
    resizes,
    reopenMs,
    reopenResizes,
    longTasks: openLong.length,
    typeMs,
  };
}

const browser = await chromium.launch({
  executablePath: process.env.PW_CHROMIUM_PATH || undefined,
});
const rows = [];
for (let i = 0; i < RUNS; i++) rows.push(await once(browser));
await browser.close();
const keys = Object.keys(rows[0]);
console.log("metric".padEnd(14), "median", "   runs");
for (const k of keys)
  console.log(
    k.padEnd(14),
    String(median(rows.map((r) => r[k])).toFixed(1)).padStart(6),
    "  ",
    rows.map((r) => r[k].toFixed(0)).join(" "),
  );
