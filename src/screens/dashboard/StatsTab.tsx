import { useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { SpeedResult } from "../../services/types";
import { formatBytes, formatRate, formatUptime, Meter, usePoll } from "./util";

export default function StatsTab() {
  const { data: s } = usePoll(() => host().statsSnapshot(), 1500);
  const [speed, setSpeed] = useState<SpeedResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [speedError, setSpeedError] = useState<string | null>(null);
  if (!s) return <div className="dash-loading" aria-busy="true" />;
  const memPct = (s.memoryUsed / Math.max(1, s.memoryTotal)) * 100;
  const runSpeed = async () => {
    setTesting(true);
    setSpeedError(null);
    try {
      setSpeed(await host().speedTest());
    } catch (e) {
      setSpeedError(toWispError(e).message);
    } finally {
      setTesting(false);
    }
  };
  return (
    <div className="stats">
      <div className="stats-grid">
        <div className="card">
          <p className="card-title">
            CPU <span className="big">{Math.round(s.cpu)}%</span>
          </p>
          <Meter value={s.cpu} label="CPU usage" />
          <div className="cores" aria-label="Per-core usage">
            {s.cores.map((c, i) => (
              <span key={i} className="core" title={`Core ${i + 1}: ${Math.round(c)}%`}>
                <span style={{ height: `${Math.max(4, c)}%` }} />
              </span>
            ))}
          </div>
          <p className="fine">{s.cpuName}</p>
        </div>
        <div className="card">
          <p className="card-title">
            Memory <span className="big">{Math.round(memPct)}%</span>
          </p>
          <Meter value={memPct} label="Memory usage" />
          <p className="fine">
            {formatBytes(s.memoryUsed)} of {formatBytes(s.memoryTotal)}
          </p>
          {s.disks.slice(0, 2).map((d) => {
            const used = ((d.totalBytes - d.freeBytes) / Math.max(1, d.totalBytes)) * 100;
            return (
              <div key={d.mount} className="disk">
                <p className="fine">
                  Disk {d.mount} · {formatBytes(d.freeBytes, 0)} free
                </p>
                <Meter value={used} label={`Disk ${d.mount} used`} />
              </div>
            );
          })}
        </div>
        <div className="card">
          <p className="card-title">Network</p>
          <p className="net">
            <span>
              <Icon name="chevron-down" size={14} /> {formatRate(s.netDown)}
            </span>
            <span>
              <Icon name="chevron-up" size={14} /> {formatRate(s.netUp)}
            </span>
          </p>
          <button className="btn btn-small" onClick={() => void runSpeed()} disabled={testing}>
            {testing ? "Testing…" : "Run speed test"}
          </button>
          {speed && (
            <p className="speed">
              <strong>{speed.mbps} Mb/s</strong> down · {speed.latencyMs} ms
            </p>
          )}
          {speedError && <p className="fine warn">{speedError}</p>}
          <p className="fine">Up {formatUptime(s.uptimeSecs)} · test uses speed.cloudflare.com</p>
        </div>
      </div>
      <table className="procs">
        <caption className="sr-only">Apps using the most memory</caption>
        <thead>
          <tr>
            <th scope="col">App</th>
            <th scope="col">Memory</th>
            <th scope="col">CPU</th>
          </tr>
        </thead>
        <tbody>
          {s.top.map((p) => (
            <tr key={p.name}>
              <td>
                {p.name}
                {p.count > 1 && <span className="muted"> ×{p.count}</span>}
              </td>
              <td>{formatBytes(p.memoryBytes)}</td>
              <td>{p.cpu.toFixed(1)}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
