import { useId } from "react";
import { ORB_LABELS, type OrbState } from "../state/orb";

interface OrbProps {
  state: OrbState;
  size?: number;
  /** Render as decorative (label is shown elsewhere). */
  decorative?: boolean;
}

/** Each state has its own glyph so status never depends on colour alone. */
function Glyph({ state }: { state: OrbState }) {
  switch (state) {
    case "listening":
      return (
        <g className="orb-glyph orb-glyph-wave">
          <rect x="41" y="44" width="4" height="12" rx="2" />
          <rect x="48" y="38" width="4" height="24" rx="2" />
          <rect x="55" y="42" width="4" height="16" rx="2" />
        </g>
      );
    case "thinking":
      return (
        <circle
          className="orb-arc"
          cx="50"
          cy="50"
          r="40"
          fill="none"
          strokeWidth="3.5"
          strokeLinecap="round"
          pathLength="100"
          strokeDasharray="22 78"
        />
      );
    case "speaking":
      return (
        <g className="orb-glyph orb-glyph-speak" fill="none" strokeWidth="3" strokeLinecap="round">
          <path d="M44 42c3 4.5 3 11.5 0 16" />
          <path d="M51 37c5 7 5 19 0 26" />
        </g>
      );
    case "success":
      return (
        <path
          className="orb-glyph orb-glyph-stroke"
          d="M38 51l8 8 16-17"
          fill="none"
          strokeWidth="5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      );
    case "warning":
      return (
        <g className="orb-glyph">
          <rect x="47" y="34" width="6" height="20" rx="3" />
          <circle cx="50" cy="63" r="3.5" />
        </g>
      );
    case "error":
      return (
        <path
          className="orb-glyph orb-glyph-stroke"
          d="M41 41l18 18M59 41L41 59"
          fill="none"
          strokeWidth="5"
          strokeLinecap="round"
        />
      );
    default:
      return <circle className="orb-glint" cx="42" cy="40" r="7" />;
  }
}

export function Orb({ state, size = 56, decorative = false }: OrbProps) {
  const id = useId().replace(/:/g, "");
  return (
    <span className={`orb orb-${state}`} style={{ width: size, height: size }} data-state={state}>
      <svg
        viewBox="0 0 100 100"
        width={size}
        height={size}
        role={decorative ? undefined : "img"}
        aria-hidden={decorative ? true : undefined}
        aria-label={decorative ? undefined : `Wisp: ${ORB_LABELS[state]}`}
      >
        <defs>
          <radialGradient id={`core-${id}`} cx="42%" cy="38%" r="65%">
            <stop offset="0%" stopColor="var(--orb-core-1)" />
            <stop offset="35%" stopColor="var(--orb-core-2)" />
            <stop offset="75%" stopColor="var(--orb-core-3)" />
            <stop offset="100%" stopColor="var(--orb-core-4)" />
          </radialGradient>
          <radialGradient id={`halo-${id}`} cx="50%" cy="50%" r="50%">
            <stop offset="55%" stopColor="var(--orb-halo)" stopOpacity="0.35" />
            <stop offset="100%" stopColor="var(--orb-halo-2)" stopOpacity="0" />
          </radialGradient>
        </defs>
        <circle className="orb-halo" cx="50" cy="50" r="49" fill={`url(#halo-${id})`} />
        <circle className="orb-core" cx="50" cy="50" r="32" fill={`url(#core-${id})`} />
        <Glyph state={state} />
      </svg>
    </span>
  );
}
