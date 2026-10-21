import { useEffect, useRef, useState, type PointerEvent } from "react";
import { Icon } from "../components/Icon";
import { toWispError, type WispError } from "../services/errors";
import { host } from "../services/host";

interface Point {
  x: number;
  y: number;
}

/**
 * Full-screen overlay on each monitor showing a frozen capture. The user drags
 * a rectangle; nothing is sent anywhere until they confirm in the island.
 */
export function SelectOverlay() {
  const monitor = host().overlayMonitor();
  const [bg, setBg] = useState<string | null>(null);
  const [start, setStart] = useState<Point | null>(null);
  const [end, setEnd] = useState<Point | null>(null);
  const [error, setError] = useState<WispError | null>(null);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    host()
      .capturePreview(monitor)
      .then(setBg)
      .catch((e) => setError(toWispError(e)));
    root.current?.focus();
  }, [monitor]);

  const cancel = () => void host().captureCancel();

  const submit = async (rect: { x: number; y: number; width: number; height: number }) => {
    try {
      await host().captureSelect(monitor, {
        ...rect,
        viewWidth: window.innerWidth,
        viewHeight: window.innerHeight,
      });
    } catch (e) {
      setError(toWispError(e));
      setStart(null);
      setEnd(null);
    }
  };

  const onDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setError(null);
    setStart({ x: e.clientX, y: e.clientY });
    setEnd({ x: e.clientX, y: e.clientY });
  };
  const onMove = (e: PointerEvent) => {
    if (start) setEnd({ x: e.clientX, y: e.clientY });
  };
  const onUp = () => {
    if (!start || !end) return;
    const rect = {
      x: Math.min(start.x, end.x),
      y: Math.min(start.y, end.y),
      width: Math.abs(end.x - start.x),
      height: Math.abs(end.y - start.y),
    };
    if (rect.width < 6 || rect.height < 6) {
      setStart(null);
      setEnd(null);
      return;
    }
    void submit(rect);
  };

  const sel =
    start && end
      ? {
          left: Math.min(start.x, end.x),
          top: Math.min(start.y, end.y),
          width: Math.abs(end.x - start.x),
          height: Math.abs(end.y - start.y),
        }
      : null;

  return (
    <div
      ref={root}
      className="overlay"
      tabIndex={-1}
      role="application"
      aria-label="Select an area of the screen. Drag to select, Enter for the whole screen, Escape to cancel."
      onKeyDown={(e) => {
        if (e.key === "Escape") cancel();
        if (e.key === "Enter")
          void submit({ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight });
      }}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      style={bg ? { backgroundImage: `url(${bg})` } : undefined}
    >
      {!sel && <div className="overlay-dim" />}
      {sel && <div className="overlay-selection" style={sel} data-testid="selection" />}
      <div className="overlay-bar" onPointerDown={(e) => e.stopPropagation()}>
        <Icon name="screen" size={16} />
        <span>{error ? error.message : "Select an area · nothing is sent until you confirm"}</span>
        <button
          className="btn btn-small"
          onClick={() =>
            void submit({ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight })
          }
        >
          Whole screen
        </button>
        <button className="btn btn-small btn-ghost" onClick={cancel}>
          Cancel <kbd>Esc</kbd>
        </button>
      </div>
    </div>
  );
}
