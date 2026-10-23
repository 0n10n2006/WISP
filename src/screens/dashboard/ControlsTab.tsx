import { useEffect, useRef, useState, type ReactNode } from "react";
import { Icon, type IconName } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type {
  ControlsState,
  LaunchItem,
  PowerMode,
  WinChange,
  WinSettingsState,
} from "../../services/types";
import { toast } from "../../state/toast";
import { usePoll } from "./util";

const SETTINGS_LINKS: { name: string; uri: string }[] = [
  { name: "Display", uri: "ms-settings:display" },
  { name: "Colours", uri: "ms-settings:colors" },
  { name: "Taskbar", uri: "ms-settings:taskbar" },
  { name: "Sound", uri: "ms-settings:sound" },
  { name: "Notifications", uri: "ms-settings:notifications" },
];

const ACCENTS: { hex: string; name: string }[] = [
  { hex: "#0078d4", name: "Blue" },
  { hex: "#00b7c3", name: "Teal" },
  { hex: "#107c10", name: "Green" },
  { hex: "#ffb900", name: "Gold" },
  { hex: "#f7630c", name: "Orange" },
  { hex: "#e81123", name: "Red" },
  { hex: "#e3008c", name: "Pink" },
  { hex: "#744da9", name: "Purple" },
];

const POWER: Record<PowerMode, { label: string; next: PowerMode }> = {
  efficiency: { label: "Efficiency", next: "balanced" },
  balanced: { label: "Balanced", next: "performance" },
  performance: { label: "Performance", next: "efficiency" },
};

function link(name: string, uri: string): LaunchItem {
  return { id: `uri:${uri}`, name, kind: "system", target: uri };
}

async function attempt(fn: () => Promise<unknown>): Promise<boolean> {
  try {
    await fn();
    return true;
  } catch (e) {
    toast(toWispError(e).message, "warning", 3500);
    return false;
  }
}

/** One square, one-tap button: icon, name and its state. */
function Tile({
  icon,
  label,
  state,
  on,
  onClick,
  disabled,
}: {
  icon: IconName;
  label: string;
  state?: string;
  on?: boolean;
  onClick(): void;
  disabled?: boolean;
}) {
  return (
    <button
      className={`qtile ${on ? "on" : ""}`}
      aria-pressed={on === undefined ? undefined : on}
      onClick={onClick}
      disabled={disabled}
      title={label}
    >
      <span className="qtile-icon">
        <Icon name={icon} size={18} />
      </span>
      <span className="qtile-label">{label}</span>
      {state && <small className="qtile-state">{state}</small>}
    </button>
  );
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="card cc-card">
      <p className="card-title">{title}</p>
      {children}
    </div>
  );
}

/** A slider that updates while dragged and isn't overwritten by polling. */
function Slider({
  label,
  value,
  icon,
  onIcon,
  onChange,
  disabled,
}: {
  label: string;
  value: number | null;
  icon: IconName;
  onIcon?(): void;
  onChange(v: number): void;
  disabled?: boolean;
}) {
  return (
    <div className="row cc-slider">
      <button
        className="icon-btn"
        aria-label={onIcon ? `${label}: toggle` : label}
        onClick={onIcon}
        disabled={!onIcon}
        tabIndex={onIcon ? 0 : -1}
      >
        <Icon name={icon} />
      </button>
      <input
        type="range"
        className="slider"
        min={0}
        max={100}
        value={value ?? 0}
        disabled={disabled || value === null}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <span className="slider-value">{value === null ? "—" : `${value}%`}</span>
    </div>
  );
}

export default function ControlsTab() {
  const controls = usePoll(() => host().controlsState(), 3000);
  const win = usePoll(() => host().winSettings(), 5000);
  const [c, setC] = useState<ControlsState | null>(null);
  const [w, setW] = useState<WinSettingsState | null>(null);
  // While the user is dragging or a change is in flight, keep our values.
  const busy = useRef(0);
  useEffect(() => {
    if (controls.data && !busy.current) setC(controls.data);
  }, [controls.data]);
  useEffect(() => {
    if (win.data && !busy.current) setW(win.data);
  }, [win.data]);

  const cs = c ?? controls.data;
  const ws = w ?? win.data;
  if (!cs) return <div className="dash-loading" aria-busy="true" />;
  if (!cs.supported)
    return (
      <p className="muted pad">
        Quick controls work on Windows. In this preview they're simulated.
      </p>
    );

  /** Change a Windows setting: update the UI now, roll back if Windows refuses. */
  const change = async (ch: WinChange, patch: Partial<WinSettingsState>) => {
    if (!ws) return;
    const before = ws;
    setW({ ...ws, ...patch });
    busy.current += 1;
    const ok = await attempt(() => host().winSettingSet(ch));
    busy.current -= 1;
    if (!ok) setW(before);
    win.refresh();
  };

  const patchControls = (p: Partial<ControlsState>) => setC({ ...cs, ...p });
  const np = cs.nowPlaying;
  const wifi = cs.radios.find((r) => r.kind === "wifi");
  const bt = cs.radios.find((r) => r.kind === "bluetooth");
  const radio = (kind: "wifi" | "bluetooth", on: boolean) => {
    patchControls({ radios: cs.radios.map((x) => (x.kind === kind ? { ...x, on } : x)) });
    void attempt(() => host().setRadio(kind, on)).then(controls.refresh);
  };
  const power = ws?.powerMode ?? null;

  return (
    <div className="controls cc">
      <div className="qtiles" role="group" aria-label="Quick settings">
        {wifi && (
          <Tile
            icon="wifi"
            label="Wi-Fi"
            state={wifi.on ? "On" : "Off"}
            on={wifi.on}
            onClick={() => radio("wifi", !wifi.on)}
          />
        )}
        {bt && (
          <Tile
            icon="bluetooth"
            label="Bluetooth"
            state={bt.on ? "On" : "Off"}
            on={bt.on}
            onClick={() => radio("bluetooth", !bt.on)}
          />
        )}
        <Tile
          icon={cs.darkMode ? "moon" : "sun"}
          label="Dark mode"
          state={cs.darkMode ? "On" : "Off"}
          on={cs.darkMode}
          onClick={() => {
            const dark = !cs.darkMode;
            patchControls({ darkMode: dark });
            void attempt(() => host().setDarkMode(dark)).then(controls.refresh);
          }}
        />
        {ws?.nightLight !== null && ws?.nightLight !== undefined && (
          <Tile
            icon="nightlight"
            label="Night light"
            state={ws.nightLight ? "On" : "Off"}
            on={ws.nightLight}
            onClick={() =>
              void change(
                { setting: "nightLight", value: !ws.nightLight },
                { nightLight: !ws.nightLight },
              )
            }
          />
        )}
        {ws?.doNotDisturb !== null && ws?.doNotDisturb !== undefined && (
          <Tile
            icon="bell-off"
            label="Do not disturb"
            state={ws.doNotDisturb ? "On" : "Off"}
            on={ws.doNotDisturb}
            onClick={() =>
              void change(
                { setting: "doNotDisturb", value: !ws.doNotDisturb },
                { doNotDisturb: !ws.doNotDisturb },
              )
            }
          />
        )}
        {ws?.micMuted !== null && ws?.micMuted !== undefined && (
          <Tile
            icon={ws.micMuted ? "mic-off" : "mic"}
            label="Microphone"
            state={ws.micMuted ? "Muted" : "On"}
            on={ws.micMuted}
            onClick={() =>
              void change({ setting: "micMute", value: !ws.micMuted }, { micMuted: !ws.micMuted })
            }
          />
        )}
        {power && (
          <Tile
            icon="gauge"
            label="Power mode"
            state={POWER[power].label}
            on={power === "performance"}
            onClick={() =>
              void change(
                { setting: "powerMode", value: POWER[power].next },
                { powerMode: POWER[power].next },
              )
            }
          />
        )}
        {ws?.transparency !== null && ws?.transparency !== undefined && (
          <Tile
            icon="layers"
            label="Transparency"
            state={ws.transparency ? "On" : "Off"}
            on={ws.transparency}
            onClick={() =>
              void change(
                { setting: "transparency", value: !ws.transparency },
                { transparency: !ws.transparency },
              )
            }
          />
        )}
        {ws?.taskbarAutoHide !== null && ws?.taskbarAutoHide !== undefined && (
          <Tile
            icon="taskbar"
            label="Hide taskbar"
            state={ws.taskbarAutoHide ? "Auto" : "Off"}
            on={ws.taskbarAutoHide}
            onClick={() =>
              void change(
                { setting: "taskbarAutoHide", value: !ws.taskbarAutoHide },
                { taskbarAutoHide: !ws.taskbarAutoHide },
              )
            }
          />
        )}
        {ws?.desktopIcons !== null && ws?.desktopIcons !== undefined && (
          <Tile
            icon="desktop"
            label="Desktop icons"
            state={ws.desktopIcons ? "Shown" : "Hidden"}
            on={ws.desktopIcons}
            onClick={() =>
              void change(
                { setting: "desktopIcons", value: !ws.desktopIcons },
                { desktopIcons: !ws.desktopIcons },
              )
            }
          />
        )}
        <Tile
          icon="shuffle"
          label="Next wallpaper"
          onClick={() =>
            void attempt(async () => {
              const p = await host().wallpaperNext(false);
              if (p) toast("Wallpaper changed", "success");
            })
          }
        />
        <Tile
          icon="lasso"
          label="Circle to ask"
          onClick={() => void attempt(() => host().startCapture())}
        />
        <Tile
          icon="lock"
          label="Lock"
          onClick={() => void attempt(() => host().powerAction("lock"))}
        />
      </div>

      <Card title="Sound & display">
        <Slider
          label="Volume"
          icon={cs.muted || cs.volume === 0 ? "mute" : "volume"}
          value={cs.volume}
          onIcon={() => {
            patchControls({ muted: !cs.muted });
            void attempt(() => host().setMuted(!cs.muted)).then(controls.refresh);
          }}
          onChange={(v) => {
            patchControls({ volume: v, muted: false });
            void attempt(() => host().setVolume(v));
          }}
        />
        {ws && ws.brightness !== null && (
          <Slider
            label="Brightness"
            icon="brightness"
            value={ws.brightness}
            onChange={(v) => {
              setW({ ...ws, brightness: v });
              void attempt(() => host().winSettingSet({ setting: "brightness", value: v }));
            }}
          />
        )}
        {ws && ws.outputs.length > 1 && (
          <div className="cc-outputs" role="radiogroup" aria-label="Sound output">
            {ws.outputs.map((o) => (
              <button
                key={o.id}
                role="radio"
                aria-checked={o.default}
                className={`chip ${o.default ? "chip-on" : ""}`}
                title={o.name}
                onClick={() =>
                  !o.default &&
                  void change(
                    { setting: "outputDevice", value: o.id },
                    { outputs: ws.outputs.map((x) => ({ ...x, default: x.id === o.id })) },
                  )
                }
              >
                <Icon name={/head|ear|buds/i.test(o.name) ? "headphones" : "speaker"} size={13} />
                {o.name.replace(/\s*\(.*\)\s*$/, "") || o.name}
              </button>
            ))}
          </div>
        )}
      </Card>

      <Card title="Now playing">
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
                onClick={() =>
                  void attempt(() => host().mediaCommand("previous")).then(controls.refresh)
                }
              >
                <Icon name="prev" />
              </button>
              <button
                className="icon-btn icon-btn-solid"
                aria-label={np.playing ? "Pause" : "Play"}
                onClick={() =>
                  void attempt(() => host().mediaCommand("playPause")).then(controls.refresh)
                }
              >
                <Icon name={np.playing ? "pause" : "play"} />
              </button>
              <button
                className="icon-btn"
                aria-label="Next track"
                onClick={() =>
                  void attempt(() => host().mediaCommand("next")).then(controls.refresh)
                }
              >
                <Icon name="next" />
              </button>
            </div>
          </div>
        ) : (
          <p className="muted">Nothing playing.</p>
        )}
      </Card>

      {ws?.supported && (
        <Card title="Appearance">
          <div className="cc-accents" role="radiogroup" aria-label="Accent colour">
            {ACCENTS.map((a) => (
              <button
                key={a.hex}
                role="radio"
                aria-checked={!ws.accentFromWallpaper && ws.accentColor === a.hex}
                aria-label={a.name}
                title={a.name}
                className="cc-swatch"
                style={{ background: a.hex }}
                onClick={() =>
                  void change(
                    { setting: "accentColor", value: a.hex },
                    { accentColor: a.hex, accentFromWallpaper: false },
                  )
                }
              />
            ))}
            <button
              className={`chip ${ws.accentFromWallpaper ? "chip-on" : ""}`}
              aria-pressed={!!ws.accentFromWallpaper}
              onClick={() =>
                void change(
                  { setting: "accentFromWallpaper", value: !ws.accentFromWallpaper },
                  { accentFromWallpaper: !ws.accentFromWallpaper },
                )
              }
            >
              Match wallpaper
            </button>
          </div>
          <div className="cc-row">
            <span className="muted">Taskbar icons</span>
            <div className="segmented" role="radiogroup" aria-label="Taskbar alignment">
              {(["left", "center"] as const).map((a) => (
                <button
                  key={a}
                  role="radio"
                  aria-checked={ws.taskbarAlign === a}
                  className={ws.taskbarAlign === a ? "on" : ""}
                  onClick={() =>
                    void change({ setting: "taskbarAlign", value: a }, { taskbarAlign: a })
                  }
                >
                  {a === "left" ? "Left" : "Centre"}
                </button>
              ))}
            </div>
          </div>
          <div className="cc-row cc-chips">
            <button
              className={`chip ${ws.accentOnTaskbar ? "chip-on" : ""}`}
              aria-pressed={!!ws.accentOnTaskbar}
              onClick={() =>
                void change(
                  { setting: "accentOnTaskbar", value: !ws.accentOnTaskbar },
                  { accentOnTaskbar: !ws.accentOnTaskbar },
                )
              }
            >
              Accent on Start &amp; taskbar
            </button>
            <button
              className={`chip ${ws.startRecent ? "chip-on" : ""}`}
              aria-pressed={!!ws.startRecent}
              onClick={() =>
                void change(
                  { setting: "startRecent", value: !ws.startRecent },
                  { startRecent: !ws.startRecent },
                )
              }
            >
              Recent items in Start
            </button>
            <button
              className={`chip ${ws.appsDark ? "chip-on" : ""}`}
              aria-pressed={!!ws.appsDark}
              onClick={() =>
                void change(
                  { setting: "appsDark", value: !ws.appsDark },
                  { appsDark: !ws.appsDark },
                )
              }
            >
              Dark apps
            </button>
            <button
              className={`chip ${ws.systemDark ? "chip-on" : ""}`}
              aria-pressed={!!ws.systemDark}
              onClick={() =>
                void change(
                  { setting: "systemDark", value: !ws.systemDark },
                  { systemDark: !ws.systemDark },
                )
              }
            >
              Dark taskbar &amp; Start
            </button>
          </div>
        </Card>
      )}

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
