// Answers to "battery", "disk space", "my ip"… from what the host reads on
// this PC. Pure functions, unit-tested; no model involved.

import type { PcTopic } from "./commands";
import type { PcInfo } from "../services/types";

const GB = 1024 ** 3;

function gb(bytes: number): string {
  const v = bytes / GB;
  return v >= 100 ? `${Math.round(v)} GB` : `${Math.round(v * 10) / 10} GB`;
}

export function duration(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const part = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"}`;
  if (d) return h ? `${part(d, "day")} ${part(h, "hour")}` : part(d, "day");
  if (h) return m ? `${part(h, "hour")} ${part(m, "minute")}` : part(h, "hour");
  return part(Math.max(m, 1), "minute");
}

/** A short Markdown answer about `topic`. */
export function pcAnswer(topic: PcTopic, info: PcInfo): string {
  const s = info.stats;
  switch (topic) {
    case "battery": {
      const b = info.battery;
      if (!b) return "This PC doesn't have a battery: it runs on mains power.";
      const state = b.charging
        ? "charging"
        : b.pluggedIn
          ? "plugged in"
          : b.secsLeft
            ? `about ${duration(b.secsLeft)} left`
            : "on battery";
      return `Battery is at **${b.percent}%** (${state}).`;
    }
    case "disk": {
      if (!s.disks.length) return "Couldn't read your drives.";
      const lines = s.disks.map((d) => {
        const used = d.totalBytes
          ? Math.round(((d.totalBytes - d.freeBytes) / d.totalBytes) * 100)
          : 0;
        return `- **${d.mount}** ${gb(d.freeBytes)} free of ${gb(d.totalBytes)} (${used}% used)`;
      });
      return ["Free space on your drives:", "", ...lines].join("\n");
    }
    case "ip":
      return info.localIp
        ? `This PC's address on your network is **${info.localIp}**. (Your public IP needs the web: ask with Web on.)`
        : "This PC isn't connected to a network right now.";
    case "memory": {
      const pct = s.memoryTotal ? Math.round((s.memoryUsed / s.memoryTotal) * 100) : 0;
      const top = [...s.top]
        .sort((a, b) => b.memoryBytes - a.memoryBytes)
        .slice(0, 3)
        .map((p) => `${p.name} (${gb(p.memoryBytes)})`);
      return (
        `Using **${gb(s.memoryUsed)}** of ${gb(s.memoryTotal)} memory (${pct}%).` +
        (top.length ? ` Most is used by ${top.join(", ")}.` : "")
      );
    }
    case "cpu": {
      const top = [...s.top]
        .sort((a, b) => b.cpu - a.cpu)
        .slice(0, 3)
        .filter((p) => p.cpu >= 0.5)
        .map((p) => `${p.name} (${Math.round(p.cpu)}%)`);
      return (
        `CPU is at **${Math.round(s.cpu)}%**${s.cpuName ? ` (${s.cpuName})` : ""}.` +
        (top.length ? ` Busiest: ${top.join(", ")}.` : "")
      );
    }
    case "uptime":
      return `This PC has been on for **${duration(s.uptimeSecs)}** since it last started.`;
  }
}
