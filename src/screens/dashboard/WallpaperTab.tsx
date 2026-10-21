import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";

const NO_WALLS: string[] = [];

function fileName(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

function Thumb({ path, current }: { path: string; current: boolean }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    host()
      .wallpaperThumb(path)
      .then((s) => alive && setSrc(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [path]);
  return (
    <li>
      <button
        className={`wall ${current ? "current" : ""}`}
        aria-pressed={current}
        title={path}
        onClick={() =>
          void host()
            .wallpaperSet(path)
            .then(() => toast("Wallpaper changed", "success"))
            .catch((e) => toast(toWispError(e).message, "warning"))
        }
      >
        {src ? <img src={src} alt="" /> : <span className="wall-ph" />}
        <span className="wall-name">{fileName(path)}</span>
      </button>
    </li>
  );
}

export default function WallpaperTab() {
  const recent = useSettings((s) => s.snapshot?.settings.recentWallpapers ?? NO_WALLS);
  const [current, setCurrent] = useState<string | null>(null);
  useEffect(() => {
    void host()
      .wallpaperInfo()
      .then((i) => setCurrent(i.current))
      .catch(() => {});
  }, [recent]);
  const pick = async () => {
    try {
      const p = await host().wallpaperPick();
      if (p) toast("Wallpaper changed", "success");
    } catch (e) {
      toast(toWispError(e).message, "warning");
    }
  };
  return (
    <div className="wallpaper">
      <div className="row">
        <p className="muted grow">
          {current ? (
            <>Current: {fileName(current)}</>
          ) : (
            "Pick an image from your PC to set it as the desktop background."
          )}
        </p>
        <button className="btn btn-small btn-primary" onClick={() => void pick()}>
          <Icon name="folder" size={14} /> Browse…
        </button>
      </div>
      {recent.length > 0 ? (
        <ul className="walls">
          {recent.map((p) => (
            <Thumb key={p} path={p} current={current === p} />
          ))}
        </ul>
      ) : (
        <p className="muted pad">Wallpapers you set here are kept for one-click switching.</p>
      )}
    </div>
  );
}
