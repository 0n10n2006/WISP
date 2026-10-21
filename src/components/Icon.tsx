// Small inline icon set (stroke icons, 20px grid). No emoji, no icon font.

export type IconName =
  | "send"
  | "stop"
  | "copy"
  | "check"
  | "close"
  | "minimize"
  | "settings"
  | "retry"
  | "warning"
  | "error"
  | "mic"
  | "screen"
  | "clipboard"
  | "eye-off"
  | "trash"
  | "external"
  | "key"
  | "info"
  | "plus"
  | "chevron-right";

const PATHS: Record<IconName, string> = {
  send: "M4 10h10M10 5l5 5-5 5",
  stop: "M6 6h8v8H6z",
  copy: "M7 7h8v9H7zM5 13H4V4h8v1",
  check: "M4.5 10.5l3.5 3.5 7.5-8",
  close: "M5 5l10 10M15 5L5 15",
  minimize: "M5 10h10",
  settings:
    "M10 7.2a2.8 2.8 0 1 1 0 5.6 2.8 2.8 0 0 1 0-5.6zM10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4",
  retry: "M15.5 8A6 6 0 1 0 16 11M16 4v4h-4",
  warning: "M10 3l7.5 13h-15zM10 8v3.5M10 14v.01",
  error: "M10 2.5a7.5 7.5 0 1 1 0 15 7.5 7.5 0 0 1 0-15zM7.5 7.5l5 5M12.5 7.5l-5 5",
  mic: "M10 3a2.5 2.5 0 0 1 2.5 2.5v4a2.5 2.5 0 0 1-5 0v-4A2.5 2.5 0 0 1 10 3zM5.5 9.5a4.5 4.5 0 0 0 9 0M10 14v3",
  screen: "M3 5h14v9H3zM7 17h6M5 3H3v2M15 3h2v2",
  clipboard: "M7 4h6v2H7zM6 5H5v12h10V5h-1M8 10h4M8 13h4",
  "eye-off":
    "M3 3l14 14M8.5 8.6a2 2 0 0 0 2.9 2.8M5.5 6C3.8 7.2 2.8 9 2.5 10c.8 2.4 3.8 5.5 7.5 5.5 1.3 0 2.5-.4 3.5-1M9 4.6c.3 0 .7-.1 1-.1 3.7 0 6.7 3.1 7.5 5.5-.3.9-.9 2-1.8 3",
  trash: "M4 6h12M8 6V4h4v2M6 6l1 11h6l1-11",
  external: "M11 4h5v5M16 4l-7 7M14 12v4H4V6h4",
  key: "M7 13a3 3 0 1 1 0-6 3 3 0 0 1 0 6zM10 10h7M14 10v2.5M16.5 10v2",
  info: "M10 2.5a7.5 7.5 0 1 1 0 15 7.5 7.5 0 0 1 0-15zM10 9v5M10 6.5v.01",
  plus: "M10 4v12M4 10h12",
  "chevron-right": "M8 5l5 5-5 5",
};

export function Icon({
  name,
  size = 18,
  title,
}: {
  name: IconName;
  size?: number;
  title?: string;
}) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title && <title>{title}</title>}
      <path d={PATHS[name]} />
    </svg>
  );
}
