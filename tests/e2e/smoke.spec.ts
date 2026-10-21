import { expect, test } from "@playwright/test";

// Browser smoke test against the mock host (no Tauri, no network).
const shots = process.env.WISP_SCREENSHOTS;

test("island: expand, ask, see streamed answer", async ({ page }) => {
  await page.goto("/?window=island");
  await page.evaluate(() => localStorage.setItem("wisp.mock.hasKey", "1"));
  await page.evaluate(() =>
    localStorage.setItem(
      "wisp.mock.settings",
      JSON.stringify({ onboardingComplete: true, theme: "dark" }),
    ),
  );
  await page.reload();
  if (shots) await page.screenshot({ path: `${shots}/island-collapsed.png` });
  await page.getByRole("button", { name: /open wisp/i }).click();
  const box = page.getByRole("textbox", { name: "Ask Wisp" });
  await expect(box).toBeFocused();
  if (shots) await page.screenshot({ path: `${shots}/island-empty.png` });
  await box.fill("Explain what Wisp is in one line");
  await box.press("Enter");
  await expect(page.getByRole("button", { name: "Copy", exact: true })).toBeVisible({
    timeout: 10_000,
  });
  if (shots) await page.screenshot({ path: `${shots}/island-answer-dark.png` });
  await box.fill("again /quota");
  await box.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Quota used up");
  if (shots) await page.screenshot({ path: `${shots}/island-quota.png` });
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
  await page.getByRole("textbox", { name: "Ask Wisp" }).fill("Hello");
  await page.getByRole("textbox", { name: "Ask Wisp" }).press("Enter");
  await expect(page.getByRole("button", { name: "Copy", exact: true })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  if (shots) await page.screenshot({ path: `${shots}/island-answer-light.png` });
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

test("select to ask: overlay and attachment", async ({ page }) => {
  await page.goto("/?window=overlay&monitor=0");
  await page.mouse.move(300, 200);
  await page.mouse.down();
  await page.mouse.move(700, 450);
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
  if (shots) await page.screenshot({ path: `${shots}/island-attachment.png` });
});

test("checkpoint 7: pin, memory toggle, model switch, larger size", async ({ page }) => {
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
  await expect(page.getByRole("button", { name: /keep wisp on top/i })).toBeVisible();
  await page.getByRole("textbox", { name: "Ask Wisp" }).fill("again /quota");
  await page.getByRole("textbox", { name: "Ask Wisp" }).press("Enter");
  await expect(page.getByRole("button", { name: /switch to .* and retry/i })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/island-model-switch.png` });

  await page.goto("/?window=settings&tab=actions");
  await page.getByRole("button", { name: /find installed apps/i }).click();
  await expect(page.getByRole("button", { name: "Allow spotify" })).toBeVisible();
  if (shots) await page.screenshot({ path: `${shots}/settings-app-picker.png`, fullPage: true });
});
