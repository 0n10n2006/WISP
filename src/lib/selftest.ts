// The UI half of `wisp --self-test`: drives the real interface through its
// own event handlers and real host calls, and checks the real window after
// each step (via the host's probe). Loaded only when the host asks for it.

import { suspendBlurClose } from "../screens/notch/control";
import { host, type SelfTestProbe, type SelfTestStep } from "../services/host";

const STEP_TIMEOUT = 6000;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Poll until `check` passes (or time runs out); returns the last value. */
async function until<T>(
  read: () => T | Promise<T>,
  check: (v: T) => boolean,
  timeout = STEP_TIMEOUT,
): Promise<{ ok: boolean; value: T }> {
  const start = Date.now();
  let value = await read();
  while (!check(value)) {
    if (Date.now() - start > timeout) return { ok: false, value };
    await sleep(50);
    value = await read();
  }
  return { ok: true, value };
}

const probe = () => host().selfTestProbe();
const $ = <T extends Element = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const $$ = <T extends Element = HTMLElement>(sel: string) => [...document.querySelectorAll<T>(sel)];
const describe = (p: SelfTestProbe) =>
  `${p.mode} ${p.width}x${p.height} at ${p.x},${p.y}${p.visible ? "" : " (hidden)"}`;

/** jsdom (tests) has no PointerEvent; a MouseEvent of the same type works. */
function pointer(type: string): Event {
  return typeof PointerEvent === "function" ? new PointerEvent(type) : new MouseEvent(type);
}

function setInput(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

class Run {
  steps: SelfTestStep[] = [];
  errors: string[] = [];
  private onError = (e: ErrorEvent) => this.errors.push(e.message);
  private onRejection = (e: PromiseRejectionEvent) =>
    this.errors.push(e.reason instanceof Error ? e.reason.message : String(e.reason));

  constructor() {
    window.addEventListener("error", this.onError);
    window.addEventListener("unhandledrejection", this.onRejection);
  }

  async step(name: string, fn: () => Promise<string | void>, optional = false) {
    const before = this.errors.length;
    try {
      const detail = (await fn()) ?? "";
      const errs = this.errors.slice(before);
      this.steps.push({
        name,
        ok: errs.length === 0,
        detail: errs.length ? `${detail} script error: ${errs.join("; ")}` : detail,
        optional,
      });
    } catch (e) {
      this.steps.push({
        name,
        ok: false,
        detail: e instanceof Error ? e.message : String(e),
        optional,
      });
    }
  }

  finish() {
    window.removeEventListener("error", this.onError);
    window.removeEventListener("unhandledrejection", this.onRejection);
    return this.steps;
  }
}

function fail(msg: string): never {
  throw new Error(msg);
}

async function island(): Promise<SelfTestStep[]> {
  const run = new Run();
  const h = host();
  // Count focus losses (reported in details) and keep them from folding the
  // bar away mid-script; headless displays move focus unpredictably.
  let blurs = 0;
  const offBlur = h.onBlur(() => (blurs += 1));
  suspendBlurClose(true);
  let sliver: SelfTestProbe | null = null;
  const notch = () => $<HTMLButtonElement>(".notch-idle");

  await run.step("sliver-on-screen", async () => {
    const r = await until(probe, (p) => p.mode === "sliver" && p.visible && p.width > 0);
    sliver = r.value;
    if (!r.ok) fail(`expected a visible sliver, got ${describe(r.value)}`);
    if (!notch()) fail("the notch button isn't rendered");
    return describe(r.value);
  });
  const base = sliver as SelfTestProbe | null;

  await run.step("hover-peeks", async () => {
    const btn = notch() ?? fail("no notch button");
    btn.dispatchEvent(pointer("pointerenter"));
    const r = await until(probe, (p) => p.mode === "peek" && p.width > (base?.width ?? 0));
    if (!r.ok) fail(`window didn't grow into the peek: ${describe(r.value)}`);
    return describe(r.value);
  });

  await run.step("leave-folds-back", async () => {
    notch()?.dispatchEvent(pointer("pointerleave"));
    const r = await until(probe, (p) => p.mode === "sliver");
    if (!r.ok) fail(`still ${describe(r.value)}`);
  });

  await run.step("click-opens-bar", async () => {
    (notch() ?? fail("no notch button")).click();
    const r = await until(
      probe,
      (p) =>
        p.mode === "open" &&
        p.visible &&
        p.width > (base?.width ?? 0) &&
        p.height > (base?.height ?? 0),
    );
    if (!r.ok) fail(`window didn't open: ${describe(r.value)}`);
    const input = await until(
      () => $("input[role=combobox]"),
      (el) => !!el,
    );
    if (!input.ok) fail("the command bar didn't render");
    return describe(r.value);
  });

  await run.step("typing-shows-results", async () => {
    const input = $<HTMLInputElement>("input[role=combobox]") ?? fail("no command bar");
    setInput(input, "spo");
    const r = await until(
      () => $$("[role=option]").length,
      (n) => n > 0,
    );
    if (!r.ok) fail("no results for 'spo'");
    const n = r.value;
    setInput(input, "");
    return `${n} results`;
  });

  await run.step("dashboard-tabs", async () => {
    const list = await until(
      () => $("[role=tablist][aria-label=Dashboard]"),
      (el) => !!el,
    );
    if (!list.ok) fail("the dashboard didn't render");
    const names: string[] = [];
    for (const tab of $$<HTMLButtonElement>("[role=tablist][aria-label=Dashboard] [role=tab]")) {
      tab.click();
      const shown = await until(
        () => ({
          selected: tab.getAttribute("aria-selected") === "true",
          loading: !!$("#dash-panel .dash-loading"),
          filled: ($("#dash-panel")?.textContent ?? "").trim().length > 0,
        }),
        (s) => s.selected && !s.loading && s.filled,
      );
      if (!shown.ok) {
        const v = shown.value;
        fail(
          `tab "${tab.textContent}" didn't render (selected: ${v.selected}, loading: ${v.loading}, ` +
            `panel text: ${v.filled}, bar open: ${!!$("input[role=combobox]")}, focus losses: ${blurs})`,
        );
      }
      names.push(tab.textContent ?? "");
    }
    return names.join(", ");
  });

  await run.step("escape-closes", async () => {
    const input = $<HTMLInputElement>("input[role=combobox]") ?? fail("no command bar");
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    const r = await until(probe, (p) => p.mode === "sliver");
    if (!r.ok) fail(`still ${describe(r.value)}`);
  });

  await run.step("shortcut-opens-focused", async () => {
    await h.selfTestAction("summon");
    const r = await until(probe, (p) => p.mode === "open" && p.visible);
    if (!r.ok) fail(`shortcut didn't open the bar: ${describe(r.value)}`);
    const input = await until(
      () => $("input[role=combobox]"),
      (el) => !!el,
    );
    if (!input.ok) fail("the command bar didn't render");
    await h.selfTestAction("close");
    const closed = await until(probe, (p) => p.mode === "sliver");
    if (!closed.ok) fail(`didn't close: ${describe(closed.value)}`);
    return r.value.focused ? "focused" : "opened (window focus not reported here)";
  });

  // Screen capture needs a real screen; on a headless machine it may not work.
  await run.step(
    "capture-overlay",
    async () => {
      await h.selfTestAction("capture");
      const shown = await until(probe, (p) => p.overlays > 0, 8000);
      await h.selfTestAction("capture-cancel");
      const gone = await until(probe, (p) => p.overlays === 0);
      if (!shown.ok) fail("no capture overlay appeared");
      if (!gone.ok) fail("the capture overlay didn't close");
      await h.selfTestAction("close");
      return "overlay shown once its screen was painted, then put away";
    },
    // Headless Linux test displays may not allow screen capture; Windows must.
    !navigator.userAgent.includes("Windows"),
  );

  await run.step("back-to-sliver", async () => {
    const r = await until(probe, (p) => p.mode === "sliver" && p.visible);
    if (!r.ok) fail(`expected the sliver, got ${describe(r.value)}`);
  });
  suspendBlurClose(false);
  offBlur();
  run.steps.push({ name: "focus-losses", ok: true, detail: String(blurs), optional: true });
  return run.finish();
}

async function settings(): Promise<SelfTestStep[]> {
  const run = new Run();
  const h = host();
  const snap = await h.getSettings();
  const wasOnboarded = snap.settings.onboardingComplete;

  await run.step("settings-renders", async () => {
    const r = await until(
      () => $$("[role=tab]").length,
      (n) => n > 0,
    );
    if (!r.ok) fail("no Settings tabs rendered");
    return `${r.value} tabs`;
  });

  if (!wasOnboarded) {
    await run.step("onboarding-renders", async () => {
      const r = await until(
        () => $("#settings-panel h2")?.textContent ?? "",
        (t) => t.length > 0,
      );
      if (!r.ok) fail("setup didn't render");
      await h.updateSettings({ onboardingComplete: true });
      const enabled = await until(
        () => $$<HTMLButtonElement>("[role=tab]").every((b) => !b.disabled),
        (v) => v,
      );
      if (!enabled.ok) fail("tabs stayed disabled after setup");
      return r.value;
    });
  }

  await run.step("every-tab", async () => {
    const seen: string[] = [];
    for (const tab of $$<HTMLButtonElement>("[role=tab]")) {
      tab.click();
      const r = await until(
        () => ({
          selected: tab.getAttribute("aria-selected") === "true",
          heading: $("#settings-panel h2")?.textContent ?? "",
        }),
        (s) => s.selected && s.heading.length > 0,
      );
      if (!r.ok) fail(`tab "${tab.textContent}" didn't render`);
      seen.push(tab.textContent ?? "");
    }
    return seen.join(", ");
  });

  if (!wasOnboarded) await h.updateSettings({ onboardingComplete: false }).catch(() => {});
  return run.finish();
}

/** Run this window's part of the scenario and send the results to the host. */
export async function runSelfTest(section: string): Promise<void> {
  // Host events reach every window; each runs only its own part.
  if (section !== host().windowKind()) return;
  const steps =
    section === "island"
      ? await island()
      : section === "settings"
        ? await settings()
        : [{ name: "unknown-section", ok: false, detail: section }];
  await host().selfTestReport(section, steps);
}
