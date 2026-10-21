import { useEffect, useRef, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { ControlsState, LaunchItem } from "../../services/types";
import { toast } from "../../state/toast";
import { usePoll } from "./util";

const SETTINGS_LINKS: { name: string; uri: string }[] = [
  { name: "Display", uri: "ms-settings:display" },
  { name: "Night light", uri: "ms-settings:nightlight" },
  { name: "Sound output", uri: "ms-settings:sound" },
  { name: "Bluetooth", uri: "ms-settings:bluetooth" },
  { name: "Wi-Fi networks", uri: "ms-settings:network-wifi" },
  { name: "Focus", uri: "ms-settings:quiethours" },
];

function link(name: string, uri: string): LaunchItem {
  return { id: `uri:${uri}`, name, kind: "system", target: uri };
}

async function attempt(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    toast(toWispError(e).message, "warning", 3000);
  }
}

export default function ControlsTab() {
  const { data, refresh } = usePoll(() => host().controlsState(), 3000);
  const [local, setLocal] = useState<ControlsState | null>(null);
  const dragging = useRef(false);
  useEffect(() => {
    if (data && !dragging.current) setLocal(data);
  }, [data]);
  const c = local ?? data;
  if (!c) return <div className="dash-loading" aria-busy="true" />;
  if (!c.supported)
    return (
      <p className="muted pad">
        Quick controls work on Windows. In this preview they're simulated.
      </p>
    );
  const patch = (p: Partial<ControlsState>) => setLocal({ ...c, ...p });
  const np = c.nowPlaying;
  return (
    <div className="controls">
      <div className="card volume-card">
        <p className="card-title">Sound</p>
        <div className="row">
          <button
            className="icon-btn"
            aria-label={c.muted ? "Unmute" : "Mute"}
            aria-pressed={c.muted}
            onClick={() => {
              patch({ muted: !c.muted });
              void attempt(() => host().setMuted(!c.muted)).then(refresh);
            }}
          >
            <Icon name={c.muted || c.volume === 0 ? "mute" : "volume"} />
          </button>
          <input
            type="range"
            className="slider"
            min={0}
            max={100}
            value={c.volume ?? 0}
            disabled={c.volume === null}
            aria-label="Volume"
            onPointerDown={() => (dragging.current = true)}
            onPointerUp={() => (dragging.current = false)}
            onChange={(e) => {
              const v = Number(e.target.value);
              patch({ volume: v, muted: false });
              void attempt(() => host().setVolume(v));
            }}
          />
          <span className="slider-value">{c.volume === null ? "—" : `${c.volume}%`}</span>
        </div>
      </div>

      <div className="card media-card">
        <p className="card-title">Now playing</p>
        {np ? (
          <div className="media">
            <div className="media-text">
              <strong>{np.title || "Unknown title"}</strong>
              <span className="muted">
                {np.artist}
                {np.app ? ` · ${np.app}` : ""}
              </span>
            </div>
            <div className="media-buttons">
              <button
                className="icon-btn"
                aria-label="Previous track"
                onClick={() => void attempt(() => host().mediaCommand("previous")).then(refresh)}
              >
                <Icon name="prev" />
              </button>
              <button
                className="icon-btn icon-btn-solid"
                aria-label={np.playing ? "Pause" : "Play"}
                onClick={() => void attempt(() => host().mediaCommand("playPause")).then(refresh)}
              >
                <Icon name={np.playing ? "pause" : "play"} />
              </button>
              <button
                className="icon-btn"
                aria-label="Next track"
                onClick={() => void attempt(() => host().mediaCommand("next")).then(refresh)}
              >
                <Icon name="next" />
              </button>
            </div>
          </div>
        ) : (
          <p className="muted">Nothing playing.</p>
        )}
      </div>

      <div className="tiles">
        {c.radios.map((r) => (
          <button
            key={r.kind}
            className={`tile ${r.on ? "on" : ""}`}
            aria-pressed={r.on}
            onClick={() => {
              patch({ radios: c.radios.map((x) => (x.kind === r.kind ? { ...x, on: !r.on } : x)) });
              void attempt(() => host().setRadio(r.kind, !r.on)).then(refresh);
            }}
          >
            <Icon name={r.kind === "wifi" ? "wifi" : "bluetooth"} />
            <span>{r.kind === "wifi" ? "Wi-Fi" : "Bluetooth"}</span>
            <small>{r.on ? "On" : "Off"}</small>
          </button>
        ))}
        <button
          className={`tile ${c.darkMode ? "on" : ""}`}
          aria-pressed={c.darkMode}
          onClick={() => {
            patch({ darkMode: !c.darkMode });
            void attempt(() => host().setDarkMode(!c.darkMode));
          }}
        >
          <Icon name={c.darkMode ? "moon" : "sun"} />
          <span>Dark mode</span>
          <small>{c.darkMode ? "On" : "Off"}</small>
        </button>
      </div>

      <div className="link-row" aria-label="Windows settings">
        {SETTINGS_LINKS.map((l) => (
          <button
            key={l.uri}
            className="btn btn-small btn-ghost"
            onClick={() => void attempt(() => host().launchItem(link(l.name, l.uri)))}
          >
            {l.name} <Icon name="external" size={12} />
          </button>
        ))}
      </div>
    </div>
  );
}
