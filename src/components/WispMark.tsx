import type { OrbState } from "../state/orb";

/** The Wisp flame, small. State is shown by motion and a ring, never colour alone. */
export function WispMark({ state = "idle", size = 22 }: { state?: OrbState; size?: number }) {
  return (
    <span className={`mark mark-${state}`} style={{ width: size, height: size }} aria-hidden>
      <svg viewBox="0 0 256 256" width={size} height={size}>
        <path
          className="mark-flame"
          d="M128 230c-46 0-78-32-78-74 0-36 22-58 42-80 14-16 24-32 22-56 34 14 66 46 80 86 6 16 12 32 12 50 0 42-32 74-78 74z"
        />
        <circle className="mark-core" cx="128" cy="160" r="40" />
        {state === "thinking" && (
          <circle
            className="mark-arc"
            cx="128"
            cy="150"
            r="112"
            fill="none"
            strokeWidth="16"
            strokeLinecap="round"
            pathLength="100"
            strokeDasharray="24 76"
          />
        )}
        {(state === "warning" || state === "error") && (
          <circle className="mark-alert" cx="214" cy="52" r="34" />
        )}
      </svg>
    </span>
  );
}
