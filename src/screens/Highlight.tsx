import { useEffect, useState } from "react";
import { host } from "../services/host";
import type { Highlight as Box } from "../services/types";

/**
 * "Show me": a click-through, transparent window over the screen that rings
 * what the model pointed at. It closes itself after a few seconds.
 */
export default function Highlight() {
  const [boxes, setBoxes] = useState<Box[]>([]);
  useEffect(() => {
    void host()
      .highlightsGet()
      .then(setBoxes)
      .catch(() => {});
  }, []);
  return (
    <div className="highlight-layer" aria-live="polite">
      {boxes.map((b, i) => (
        <div
          key={i}
          className="highlight-box"
          style={{
            left: `${b.x * 100}%`,
            top: `${b.y * 100}%`,
            width: `${b.width * 100}%`,
            height: `${b.height * 100}%`,
          }}
        >
          <span className={`highlight-label ${b.y < 0.08 ? "below" : ""}`}>{b.label}</span>
        </div>
      ))}
    </div>
  );
}
