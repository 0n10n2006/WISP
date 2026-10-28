// Executes command-bar results. Everything here goes through typed host calls.

import type { BarResult, Command } from "../../lib/commands";
import { formatMinutes } from "../../lib/commands";
import { pcAnswer } from "../../lib/pcinfo";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import { useConversation } from "../../state/conversation";
import { useVoice } from "../../state/voice";
import { useNotch } from "../../state/notch";
import { toast } from "../../state/toast";

export { useConfirm } from "./confirm";
import { useConfirm } from "./confirm";

async function guarded(fn: () => Promise<unknown>, done?: string) {
  try {
    await fn();
    if (done) toast(done, "success");
  } catch (e) {
    toast(toWispError(e).message, "warning", 3500);
  }
}

export async function runCommand(cmd: Command): Promise<void> {
  const h = host();
  const notch = useNotch.getState();
  switch (cmd.type) {
    case "power":
      return guarded(
        () => h.powerAction(cmd.action),
        cmd.action === "restart" || cmd.action === "shutdown"
          ? "Starting in 10 seconds. Type “cancel shutdown” to stop it."
          : cmd.action === "cancel"
            ? "Cancelled"
            : undefined,
      );
    case "timer":
      await notch.startTimer(cmd.minutes);
      toast(`Focus timer: ${formatMinutes(cmd.minutes)}`, "success");
      return;
    case "stopTimer":
      await notch.stopTimer();
      toast("Timer stopped", "neutral");
      return;
    case "remind":
      return guarded(
        () =>
          h.taskCreate({
            title: cmd.title,
            dueAt: new Date(Date.now() + cmd.inMinutes * 60_000).toISOString(),
            notificationEnabled: true,
          }),
        `Reminder set for ${formatMinutes(cmd.inMinutes)} from now`,
      );
    case "volume":
      return guarded(() => h.setVolume(cmd.percent), `Volume ${cmd.percent}%`);
    case "mute":
      return guarded(() => h.setMuted(cmd.muted), cmd.muted ? "Muted" : "Unmuted");
    case "media":
      return guarded(() => h.mediaCommand(cmd.command));
    case "darkMode":
      return guarded(() => h.setDarkMode(cmd.dark), cmd.dark ? "Dark mode on" : "Light mode on");
    case "winSetting":
      return guarded(() => h.winSettingSet(cmd.change), cmd.done);
    case "wallpaper":
      return guarded(() => h.wallpaperNext(cmd.shuffle), "Wallpaper changed");
    case "radio":
      return guarded(
        () => h.setRadio(cmd.kind, cmd.on),
        `${cmd.kind === "wifi" ? "Wi-Fi" : "Bluetooth"} ${cmd.on ? "on" : "off"}`,
      );
    case "tab":
      notch.setTab(cmd.tab);
      notch.setQuery("");
      return;
    case "settings":
      return guarded(() => h.openSettings(cmd.tab));
    case "showMe":
      return guarded(async () => {
        toast("Looking at your screen…", "neutral", 2500);
        const found = await h.showMe(cmd.question);
        toast(
          found.length
            ? `Found: ${found.map((f) => f.label).join(", ")}`
            : "Couldn't spot that on screen",
          found.length ? "success" : "warning",
          3500,
        );
      });
    case "research":
      return guarded(async () => {
        await h.agentStart({ kind: "research", topic: cmd.topic });
        notch.setTab("agents");
      }, "Research agent started");
    case "replica":
      return guarded(async () => {
        await h.agentStart({ kind: "replica" });
        notch.setTab("agents");
      }, "Replica agent started");
    case "selectArea":
      return guarded(() => h.startCapture());
    case "dictate":
      return void useVoice.getState().dictate();
    case "tuck":
      return guarded(
        () => h.setTucked(true),
        "Tucked away. Use the tray or shortcut to bring Wisp back.",
      );
    case "clearChat":
      useConversation.getState().clear();
      return;
    case "pcInfo":
      return guarded(async () => {
        const info = await h.pcInfo();
        useConversation.getState().localAnswer(cmd.question, pcAnswer(cmd.topic, info));
      });
  }
}

/** Run a bar result (Enter or click). */
export async function runResult(
  r: BarResult,
  opts: { web: boolean; think: boolean; alt?: boolean },
): Promise<void> {
  const notch = useNotch.getState();
  if (r.kind === "ask") {
    notch.setQuery("");
    notch.setWeb(false);
    notch.setThink(false);
    await useConversation.getState().ask(r.prompt, { web: opts.web, think: opts.think });
    return;
  }
  if (r.kind === "app") {
    notch.setQuery("");
    await guarded(async () => {
      await host().launchItem(r.item);
      await host().hideIsland();
    }, `Opening ${r.title}`);
    return;
  }
  if (r.kind === "window") {
    notch.setQuery("");
    await guarded(() => host().switchWindow(r.window.id));
    return;
  }
  if (r.kind === "file") {
    notch.setQuery("");
    await guarded(
      async () => {
        await (opts.alt ? host().fileReveal(r.file.id) : host().fileOpen(r.file.id));
        await host().hideIsland();
      },
      opts.alt ? `Showing ${r.title}` : `Opening ${r.title}`,
    );
    return;
  }
  if (r.kind === "calc") {
    if (!r.calc.copy) return;
    await guarded(() => host().copyText(r.calc.copy), `Copied ${r.calc.copy}`);
    return;
  }
  if (r.kind === "web") {
    notch.setQuery("");
    await guarded(async () => {
      await host().openExternal(r.url);
      await host().hideIsland();
    });
    return;
  }
  if (r.kind === "tab") {
    notch.setQuery("");
    await guarded(() => host().browserTabSwitch(r.tab.id));
    return;
  }
  if (r.kind === "history") {
    notch.setQuery("");
    await guarded(async () => {
      await host().browserOpen(r.item.id);
      await host().hideIsland();
    });
    return;
  }
  if (r.kind === "process") {
    const p = r.proc;
    if (!opts.alt) {
      if (p.window !== null) await guarded(() => host().switchWindow(p.window!));
      else toast(`${p.name} has no window. Ctrl+Enter ends it.`, "neutral", 3000);
      return;
    }
    if (p.protected) {
      toast(`Wisp won't end ${p.name}: Windows needs it.`, "warning", 3500);
      return;
    }
    const warn =
      p.name.toLowerCase() === "explorer"
        ? "End Explorer? The taskbar and desktop disappear until it restarts."
        : `End ${p.name}${p.count > 1 ? ` (${p.count} processes)` : ""}? Unsaved work in it is lost.`;
    useConfirm.getState().ask(warn, `End ${p.name}`, async () => {
      await guarded(() => host().processEnd(p.name), `Ended ${p.name}`);
    });
    return;
  }
  notch.setQuery("");
  if (r.confirm) {
    useConfirm.getState().ask(r.confirm, r.title, () => runCommand(r.command));
    return;
  }
  await runCommand(r.command);
}
