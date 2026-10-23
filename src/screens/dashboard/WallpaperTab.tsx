import { useEffect, useRef, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";

const NO_WALLS: string[] = [];

function fileName(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

/** A preview that loads only once it scrolls into view. */
function Thumb({
  path,
  current,
  onPick,
}: {
  path: string;
  current: boolean;
  onPick(path: string): void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [visible, setVisible] = useState(typeof IntersectionObserver === "undefined");
  useEffect(() => {
    const el = ref.current;
    if (visible || !el) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisible(true);
          io.disconnect();
        }
      },
      { rootMargin: "0px 200px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [visible]);
  useEffect(() => {
    if (!visible) return;
    let alive = true;
    host()
      .wallpaperThumb(path)
      .then((s) => alive && setSrc(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [path, visible]);
  return (
    <li>
      <button
        ref={ref}
        className={`wall ${current ? "current" : ""}`}
        aria-pressed={current}
        title={path}
        onClick={() => onPick(path)}
      >
        {src ? <img src={src} alt="" /> : <span className="wall-ph" />}
        <span className="wall-name">{fileName(path)}</span>
      </button>
    </li>
  );
}

export default function WallpaperTab() {
  const recent = useSettings((s) => s.snapshot?.settings.recentWallpapers ?? NO_WALLS);
  const folders = useSettings((s) => s.snapshot?.settings.wallpaperFolders ?? NO_WALLS);
  const [current, setCurrent] = useState<string | null>(null);
  const [images, setImages] = useState<string[]>([]);
  const strip = useRef<HTMLUListElement>(null);

  useEffect(() => {
    void host()
      .wallpaperInfo()
      .then((i) => setCurrent(i.current))
      .catch(() => {});
  }, [recent]);
  useEffect(() => {
    let alive = true;
    host()
      .wallpaperList()
      .then((l) => alive && setImages(l))
      .catch(() => alive && setImages([]));
    return () => {
      alive = false;
    };
  }, [folders]);

  const run = async (fn: () => Promise<string | null | void>) => {
    try {
      const p = await fn();
      if (typeof p === "string") {
        setCurrent(p);
        toast("Wallpaper changed", "success");
      }
    } catch (e) {
      toast(toWispError(e).message, "warning");
    }
  };
  const pick = (path: string) =>
    void run(async () => {
      await host().wallpaperSet(path);
      return path;
    });

  const list = images.length ? images : recent;
  // Keep the current wallpaper in view.
  useEffect(() => {
    const el = strip.current?.querySelector<HTMLElement>(".wall.current");
    el?.scrollIntoView?.({ block: "nearest", inline: "center" });
  }, [current, list.length]);

  return (
    <div className="wallpaper">
      <div className="wall-bar">
        <p className="muted grow">
          {current ? <>Now: {fileName(current)}</> : "Pick an image or add a folder of wallpapers."}
        </p>
        <button
          className="btn btn-small"
          disabled={list.length < 2}
          onClick={() => void run(() => host().wallpaperNext(false))}
        >
          <Icon name="next" size={14} /> Next
        </button>
        <button
          className="btn btn-small"
          disabled={list.length < 2}
          onClick={() => void run(() => host().wallpaperNext(true))}
        >
          <Icon name="shuffle" size={14} /> Shuffle
        </button>
        <button
          className="btn btn-small btn-primary"
          onClick={() => void run(async () => void (await host().wallpaperFolderAdd()))}
        >
          <Icon name="folder" size={14} /> Add folder
        </button>
        <button
          className="btn btn-small btn-ghost"
          onClick={() => void run(() => host().wallpaperPick())}
        >
          Browse…
        </button>
      </div>
      {folders.length > 0 && (
        <ul className="app-list wall-folders" aria-label="Wallpaper folders">
          {folders.map((f) => (
            <li key={f} className="app-chip" title={f}>
              <Icon name="folder" size={12} /> {fileName(f) || f}
              <button
                className="icon-btn icon-btn-small"
                aria-label={`Remove folder ${fileName(f)}`}
                onClick={() => void run(async () => void (await host().wallpaperFolderRemove(f)))}
              >
                <Icon name="close" size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {list.length > 0 ? (
        <ul
          ref={strip}
          className="walls wall-strip"
          aria-label="Wallpapers"
          onWheel={(e) => {
            // Scroll the strip sideways with an ordinary mouse wheel.
            if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) e.currentTarget.scrollLeft += e.deltaY;
          }}
          onKeyDown={(e) => {
            if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
            const items = [...e.currentTarget.querySelectorAll<HTMLButtonElement>(".wall")];
            const at = items.indexOf(document.activeElement as HTMLButtonElement);
            const next =
              items[
                Math.max(0, Math.min(items.length - 1, at + (e.key === "ArrowRight" ? 1 : -1)))
              ];
            next?.focus();
            e.preventDefault();
          }}
        >
          {list.map((p) => (
            <Thumb key={p} path={p} current={current === p} onPick={pick} />
          ))}
        </ul>
      ) : (
        <p className="muted pad">
          Add a folder (for example Pictures\Wallpapers) to flip through it here, or pick single
          images with Browse.
        </p>
      )}
    </div>
  );
}
