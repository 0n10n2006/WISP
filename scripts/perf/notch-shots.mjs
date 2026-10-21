// Screenshots of every notch state and dashboard tab (plus Settings) from
// the production build, for visual review. Usage:
//   npx vite preview --port 4173 &
//   node scripts/perf/notch-shots.mjs <out-dir> [dark|light]
import { chromium } from "@playwright/test";
const out = process.argv[2];
const theme = process.argv[3] || "dark";
const b = await chromium.launch({ executablePath: process.env.PW_CHROMIUM_PATH || undefined });
const p = await b.newPage({ viewport: { width: 1100, height: 720 }, colorScheme: theme });
await p.addInitScript((t) => {
  localStorage.setItem("wisp.mock.hasKey", "1");
  localStorage.setItem(
    "wisp.mock.settings",
    JSON.stringify({
      onboardingComplete: true,
      theme: "system",
      clipboardHistory: true,
      weatherPlace: { name: "London", latitude: 51.5, longitude: -0.1 },
    }),
  );
}, theme);
await p.goto("http://localhost:4173/");
await p.waitForTimeout(800);
await p.screenshot({
  path: `${out}/${theme}-1-sliver.png`,
  clip: { x: 350, y: 0, width: 400, height: 80 },
});
await p.hover(".notch-idle");
await p.waitForTimeout(400);
await p.screenshot({
  path: `${out}/${theme}-2-peek.png`,
  clip: { x: 300, y: 0, width: 500, height: 100 },
});
await p.click(".notch-idle");
await p.waitForTimeout(600);
await p.screenshot({ path: `${out}/${theme}-3-today.png` });
for (const t of ["controls", "stats", "clipboard", "apps", "agents", "power"]) {
  await p.click(`#dash-tab-${t}`);
  await p.waitForTimeout(500);
  await p.screenshot({ path: `${out}/${theme}-4-${t}.png` });
}
await p.fill(".cmd-input", "spo");
await p.waitForTimeout(200);
await p.screenshot({ path: `${out}/${theme}-5-search.png` });
const sp = await b.newPage({ viewport: { width: 880, height: 620 }, colorScheme: theme });
await sp.addInitScript(() => {
  localStorage.setItem("wisp.mock.hasKey", "1");
  localStorage.setItem("wisp.mock.settings", JSON.stringify({ onboardingComplete: true }));
});
await sp.goto("http://localhost:4173/?window=settings");
await sp.waitForTimeout(700);
await sp.screenshot({ path: `${out}/${theme}-6-settings.png` });
await b.close();
