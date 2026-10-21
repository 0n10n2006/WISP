import { useEffect, useRef, useState } from "react";
import { toWispError, type WispError } from "../../services/errors";

/** Poll `load` every `ms` while mounted (tabs unmount when hidden). */
export function usePoll<T>(
  load: () => Promise<T>,
  ms: number,
): {
  data: T | null;
  error: WispError | null;
  refresh(): void;
} {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<WispError | null>(null);
  const fn = useRef(load);
  useEffect(() => {
    fn.current = load;
  });
  const [n, setN] = useState(0);
  useEffect(() => {
    let alive = true;
    const run = () =>
      fn
        .current()
        .then((d) => {
          if (alive) {
            setData(d);
            setError(null);
          }
        })
        .catch((e) => alive && setError(toWispError(e)));
    void run();
    const t = ms > 0 ? setInterval(() => void run(), ms) : null;
    return () => {
      alive = false;
      if (t) clearInterval(t);
    };
  }, [ms, n]);
  return { data, error, refresh: () => setN((x) => x + 1) };
}

export function formatBytes(n: number, digits = 1): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function formatRate(bytesPerSec: number): string {
  const bits = bytesPerSec * 8;
  if (bits >= 1e9) return `${(bits / 1e9).toFixed(1)} Gb/s`;
  if (bits >= 1e6) return `${(bits / 1e6).toFixed(1)} Mb/s`;
  if (bits >= 1e3) return `${(bits / 1e3).toFixed(0)} kb/s`;
  return `${bits.toFixed(0)} b/s`;
}

export function timeAgo(iso: string, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - new Date(iso).getTime()) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  return new Date(iso).toLocaleDateString([], { day: "numeric", month: "short" });
}

export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d ? `${d}d ${h}h` : h ? `${h}h ${m}m` : `${m}m`;
}

export function Meter({ value, label }: { value: number; label: string }) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div
      className={`meter ${v > 85 ? "meter-hot" : ""}`}
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v)}
    >
      <span style={{ width: `${v}%` }} />
    </div>
  );
}
