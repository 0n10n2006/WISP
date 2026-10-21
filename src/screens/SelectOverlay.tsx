import { useEffect, useRef, useState, type PointerEvent } from "react";
import { Icon } from "../components/Icon";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";
import type { Point } from "../services/types";

type Mode = "lasso" | "rect";
const MODE_KEY = "wisp.overlay.mode";

function initialMode(): Mode {
  try {
    return localStorage.getItem(MODE_KEY) === "rect" ? "rect" : "lasso";
  } catch {
    return "lasso";
  }
}

/** Bounding box of a path. */
export function pathBounds(points: Point[]) {
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
}

/**
 * Full-screen overlay on each monitor showing a frozen capture. The user
 * circles (lasso) or drags a rectangle; nothing is sent anywhere until they
 * confirm in the bar.
 */
export function SelectOverlay() {
  const monitor = host().overlayMonitor();
  const [bg, setBg] = useState<string | null>(null);
  const [mode, setMode] = useState<Mode>(initialMode);
  const [start, setStart] = useState<Point | null>(null);
  const [end, setEnd] = useState<Point | null>(null);
  const [path, setPath] = useState<Point[]>([]);
  const [drawing, setDrawing] = useState(false);
  const [error, setError] = useState<WispError | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    host()
      .capturePreview(monitor)
      .then(setBg)
      .catch((e) => setError(toWispError(e)));
    root.current?.focus();
  }, [monitor]);

  const switchMode = (m: Mode) => {
    setMode(m);
    reset();
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {
      /* ignore */
    }
  };
  const reset = () => {
    setStart(null);
    setEnd(null);
    setPath([]);
    setDrawing(false);
  };
  const cancel = () => void host().captureCancel();

  const view = () => ({ viewWidth: window.innerWidth, viewHeight: window.innerHeight });
  const submitRect = async (rect: { x: number; y: number; width: number; height: number }) => {
    try {
      await host().captureSelect(monitor, { ...rect, ...view() });
    } catch (e) {
      setError(toWispError(e));
      reset();
    }
  };
  const submitLasso = async (points: Point[]) => {
    try {
      const { viewWidth, viewHeight } = view();
      await host().captureSelectLasso(monitor, points, viewWidth, viewHeight);
    } catch (e) {
      setError(toWispError(e));
      reset();
    }
  };
  const whole = () =>
    void submitRect({ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight });

  const onDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setError(null);
    const p = { x: e.clientX, y: e.clientY };
    if (mode === "lasso") {
      setPath([p]);
      setDrawing(true);
    } else {
      setStart(p);
      setEnd(p);
    }
  };
  const onMove = (e: PointerEvent) => {
    const p = { x: e.clientX, y: e.clientY };
    if (mode === "lasso") {
      if (!drawing) return;
      setPath((prev) => {
        const last = prev[prev.length - 1];
        if (last && Math.hypot(p.x - last.x, p.y - last.y) < 3) return prev;
        return prev.length >= 4000 ? prev : [...prev, p];
      });
    } else if (start) setEnd(p);
  };
  const onUp = () => {
    if (mode === "lasso") {
      setDrawing(false);
      const b = path.length >= 3 ? pathBounds(path) : null;
      if (!b || b.width < 8 || b.height < 8) return reset();
      void submitLasso(path);
      return;
    }
    if (!start || !end) return;
    const rect = {
      x: Math.min(start.x, end.x),
      y: Math.min(start.y, end.y),
      width: Math.abs(end.x - start.x),
      height: Math.abs(end.y - start.y),
    };
    if (rect.width < 6 || rect.height < 6) return reset();
    void submitRect(rect);
  };

  const sel =
    mode === "rect" && start && end
      ? {
          left: Math.min(start.x, end.x),
          top: Math.min(start.y, end.y),
          width: Math.abs(end.x - start.x),
          height: Math.abs(end.y - start.y),
        }
      : null;
  const points = path.map((p) => `${p.x},${p.y}`).join(" ");

  return (
    <div
      ref={root}
      className="overlay"
      tabIndex={-1}
      role="application"
      aria-label="Circle or select an area of the screen. Enter for the whole screen, R for rectangle, L for lasso, Escape to cancel."
      onKeyDown={(e) => {
        if (e.key === "Escape") cancel();
        if (e.key === "Enter") whole();
        if (e.key.toLowerCase() === "r") switchMode("rect");
        if (e.key.toLowerCase() === "l") switchMode("lasso");
      }}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      style={bg ? { backgroundImage: `url(${bg})` } : undefined}
    >
      {!sel && path.length < 2 && <div className="overlay-dim" />}
      {sel && <div className="overlay-selection" style={sel} data-testid="selection" />}
      {path.length >= 2 && (
        <svg className="overlay-lasso" width="100%" height="100%" data-testid="lasso">
          <defs>
            <mask id="lasso-mask">
              <rect width="100%" height="100%" fill="white" />
              <polygon points={points} fill="black" />
            </mask>
          </defs>
          <rect width="100%" height="100%" className="lasso-dim" mask="url(#lasso-mask)" />
          <polyline
            points={drawing ? points : `${points} ${path[0]!.x},${path[0]!.y}`}
            className="lasso-line"
          />
        </svg>
      )}
      <div className="overlay-bar" onPointerDown={(e) => e.stopPropagation()}>
        <div className="segmented segmented-dark" role="radiogroup" aria-label="Selection shape">
          <button
            role="radio"
            aria-checked={mode === "lasso"}
            className={mode === "lasso" ? "on" : ""}
            onClick={() => switchMode("lasso")}
            title="Circle (L)"
          >
            <Icon name="lasso" size={15} /> Circle
          </button>
          <button
            role="radio"
            aria-checked={mode === "rect"}
            className={mode === "rect" ? "on" : ""}
            onClick={() => switchMode("rect")}
            title="Rectangle (R)"
          >
            <Icon name="screen" size={15} /> Box
          </button>
        </div>
        <span>
          {error
            ? error.message
            : mode === "lasso"
              ? "Circle what you want to ask about · nothing is sent until you confirm"
              : "Drag a box · nothing is sent until you confirm"}
        </span>
        <button className="btn btn-small" onClick={whole}>
          Whole screen
        </button>
        <button className="btn btn-small btn-ghost" onClick={cancel}>
          Cancel <kbd>Esc</kbd>
        </button>
      </div>
    </div>
  );
}
