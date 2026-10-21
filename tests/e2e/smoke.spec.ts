import { expect, test } from "@playwright/test";

// Browser smoke test against the mock host (no Tauri, no network).
const shots = process.env.WISP_SCREENSHOTS;

test("notch: peek, open, ask, see streamed answer", async ({ page }) => {
  await page.goto("/?window=island");
  await page.evaluate(() => localStorage.setItem("wisp.mock.hasKey", "1"));
  await page.evaluate(() =>
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "dark" }),
    ),
  );
  await page.reload();
  if (shots) await page.screenshot({ path: `${shots}/notch-sliver.png` });
  await page.getByRole("button", { name: /open wisp/i }).hover();
  await expect(page.getByTestId("island-window")).toHaveAttribute("data-mode", "peek");
  if (shots) await page.screenshot({ path: `${shots}/notch-peek.png` });
  await page.getByRole("button", { name: /open wisp/i }).click();
  const box = page.getByRole("combobox", { name: /ask wisp/i });
  await expect(box).toBeFocused();
  if (shots) await page.screenshot({ path: `${shots}/notch-open-today.png` });
  await box.fill("Explain what Wisp is in one line");
  await box.press("Enter");
  await expect(page.getByRole("button", { name: "Copy", exact: true })).toBeVisible({
    timeout: 10_000,
  });
  if (shots) await page.screenshot({ path: `${shots}/notch-answer-dark.png` });
  await box.fill("again /quota");
  await box.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Quota used up");
  if (shots) await page.screenshot({ path: `${shots}/notch-quota.png` });
});

test("light theme renders", async ({ page }) => {
  await page.goto("/?window=island");
  await page.evaluate(() => {
    localStorage.setItem("wisp.mock.hasKey", "1");
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "light" }),
    );
  });
  await page.reload();
  await page.getByRole("button", { name: /open wisp/i }).click();
  await page.getByRole("combobox", { name: /ask wisp/i }).fill("Hello");
  await page.getByRole("combobox", { name: /ask wisp/i }).press("Enter");
  await expect(page.getByRole("button", { name: "Copy", exact: true })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  if (shots) await page.screenshot({ path: `${shots}/notch-answer-light.png` });
});

test("settings: onboarding is shown first", async ({ page }) => {
  await page.goto("/?window=settings");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.getByRole("heading", { name: "Meet Wisp" })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/settings-onboarding.png`, fullPage: true });
  await page.getByRole("button", { name: /continue/i }).click();
  await page.getByLabel("API key").fill("AIzaPREVIEW-000000000000000000000");
  await page.getByRole("button", { name: /test & save/i }).click();
  await page.getByRole("button", { name: /finish setup/i }).click();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(page.getByRole("heading", { name: "General" })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/settings-general.png`, fullPage: true });
  await page.getByRole("tab", { name: "Shortcuts" }).click();
  if (shots) await page.screenshot({ path: `${shots}/settings-shortcuts.png`, fullPage: true });
  await page.getByRole("tab", { name: "Privacy" }).click();
  if (shots) await page.screenshot({ path: `${shots}/settings-privacy.png`, fullPage: true });
});

test("circle to ask: overlay and attachment", async ({ page }) => {
  await page.goto("/?window=overlay&monitor=0");
  await page.mouse.move(300, 200);
  await page.mouse.down();
  const path: [number, number][] = [
    [500, 160],
    [700, 300],
    [620, 460],
    [360, 430],
    [290, 260],
  ];
  for (const [x, y] of path) await page.mouse.move(x, y, { steps: 4 });
  if (shots) await page.screenshot({ path: `${shots}/overlay-selecting.png` });
  await page.mouse.up();

  await page.goto("/?window=island");
  await page.evaluate(() => {
    localStorage.setItem("wisp.mock.hasKey", "1");
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "dark" }),
    );
  });
  await page.reload();
  await page.getByRole("button", { name: /open wisp/i }).click();
  await page.getByRole("button", { name: /select an area/i }).click();
  await expect(page.getByText("Screen area attached")).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/notch-attachment.png` });
});

test("checkpoint 7: memory toggle, model switch, larger size", async ({ page }) => {
  await page.goto("/?window=island");
  await page.evaluate(() => {
    localStorage.setItem("wisp.mock.hasKey", "1");
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "dark", islandScale: 1.15 }),
    );
  });
  await page.reload();
  await page.getByRole("button", { name: /open wisp/i }).click();
  await expect(
    page.getByRole("button", { name: /use memory in this conversation/i }),
  ).toBeVisible();
  await page.getByRole("combobox", { name: /ask wisp/i }).fill("again /quota");
  await page.getByRole("combobox", { name: /ask wisp/i }).press("Enter");
  await expect(page.getByRole("button", { name: /switch to .* and retry/i })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/notch-model-switch.png` });

  await page.goto("/?window=settings&tab=actions");
  await page.getByRole("button", { name: /find installed apps/i }).click();
  await expect(page.getByRole("button", { name: "Allow spotify" })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/settings-app-picker.png`, fullPage: true });
});

test("command bar and every dashboard tab", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem("wisp.mock.hasKey", "1");
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({
        onboardingComplete: true,
        theme: "dark",
        clipboardHistory: true,
        weatherPlace: { name: "London, England", latitude: 51.5, longitude: -0.12 },
      }),
    );
  });
  await page.reload();
  await page.getByRole("button", { name: /open wisp/i }).click();
  for (const tab of [
    "Today",
    "Controls",
    "Stats",
    "Clipboard",
    "Apps",
    "Agents",
    "Wallpaper",
    "Power",
  ]) {
    await page.getByRole("tab", { name: tab }).click();
    await page.waitForTimeout(400);
    if (shots) await page.screenshot({ path: `${shots}/dash-${tab.toLowerCase()}.png` });
  }
  const box = page.getByRole("combobox", { name: /ask wisp/i });
  await box.fill("spo");
  await expect(page.getByRole("option", { name: /spotify/i })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/bar-apps.png` });
  await box.fill("timer 25");
  await expect(page.getByRole("option", { name: /focus timer/i })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/bar-command.png` });
  await box.fill("latest wisp news?");
  await box.press("Tab");
  await box.press("Enter");
  await expect(page.getByRole("button", { name: /wikipedia/i })).toBeVisible({ timeout: 10_000 });
  if (shots) await page.screenshot({ path: `${shots}/bar-web-answer.png` });
});
