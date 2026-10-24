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
  | "chevron-right"
  | "speaker"
  | "pin"
  | "memory"
  | "talk"
  | "globe"
  | "spark"
  | "sun"
  | "moon"
  | "wifi"
  | "bluetooth"
  | "volume"
  | "mute"
  | "play"
  | "pause"
  | "next"
  | "prev"
  | "cpu"
  | "grid"
  | "bot"
  | "image"
  | "power"
  | "lock"
  | "sleep"
  | "restart"
  | "star"
  | "link"
  | "calendar"
  | "timer"
  | "search"
  | "chevron-up"
  | "chevron-down"
  | "lasso"
  | "target"
  | "folder"
  | "today"
  | "sliders"
  | "logout"
  | "cloud"
  | "rain"
  | "snow"
  | "storm"
  | "fog"
  | "partly"
  | "nightlight"
  | "bell"
  | "bell-off"
  | "layers"
  | "taskbar"
  | "desktop"
  | "shuffle"
  | "gauge"
  | "mic-off"
  | "brightness"
  | "headphones";

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
  pin: "M7.5 3h5l-1 5 3 3H5.5l3-3zM10 11v6",
  memory: "M6 3h8a1 1 0 0 1 1 1v13l-5-3-5 3V4a1 1 0 0 1 1-1z",
  speaker: "M3 8h3l4-3.5v11L6 12H3zM13.5 7.5a3.5 3.5 0 0 1 0 5M15.5 5.5a6.5 6.5 0 0 1 0 9",
  talk: "M4 4h12v9H9l-4 3v-3H4zM7.5 8.5h.01M10 8.5h.01M12.5 8.5h.01",
  globe:
    "M10 2.5a7.5 7.5 0 1 1 0 15 7.5 7.5 0 0 1 0-15zM2.5 10h15M10 2.5c2 2.2 3 4.7 3 7.5s-1 5.3-3 7.5c-2-2.2-3-4.7-3-7.5s1-5.3 3-7.5z",
  spark:
    "M10 2.5l1.6 4.4 4.4 1.6-4.4 1.6L10 14.5l-1.6-4.4L4 8.5l4.4-1.6zM15.5 13l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z",
  sun: "M10 6.5a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7zM10 2v1.5M10 16.5V18M2 10h1.5M16.5 10H18M4.3 4.3l1.1 1.1M14.6 14.6l1.1 1.1M4.3 15.7l1.1-1.1M14.6 5.4l1.1-1.1",
  moon: "M15.5 12.5A6.5 6.5 0 0 1 7.5 4.5a6.5 6.5 0 1 0 8 8z",
  wifi: "M2.5 7.5a11 11 0 0 1 15 0M5 10.3a7.3 7.3 0 0 1 10 0M7.5 13a3.6 3.6 0 0 1 5 0M10 16v.01",
  bluetooth: "M6 6.5l8 7-4 3.5V3l4 3.5-8 7",
  volume: "M3 8h3l4-3.5v11L6 12H3zM13.5 7.5a3.5 3.5 0 0 1 0 5",
  mute: "M3 8h3l4-3.5v11L6 12H3zM13 8l4 4M17 8l-4 4",
  play: "M6.5 4.5v11l9-5.5z",
  pause: "M6.5 4.5v11M13.5 4.5v11",
  next: "M5 4.5v11l8-5.5zM15 4.5v11",
  prev: "M15 4.5v11l-8-5.5zM5 4.5v11",
  cpu: "M6 6h8v8H6zM8 2.5V6M12 2.5V6M8 14v3.5M12 14v3.5M2.5 8H6M2.5 12H6M14 8h3.5M14 12h3.5",
  grid: "M4 4h4.5v4.5H4zM11.5 4H16v4.5h-4.5zM4 11.5h4.5V16H4zM11.5 11.5H16V16h-4.5z",
  bot: "M5 7h10v8H5zM10 3.5V7M7.5 10.5v.01M12.5 10.5v.01M8 13h4M3 10v2M17 10v2",
  image: "M3 4h14v12H3zM3 13l4-4 3 3 2-2 5 5M13 7.5v.01",
  power: "M10 2.5V10M6 5a6.5 6.5 0 1 0 8 0",
  lock: "M5 9h10v8H5zM7 9V6.5a3 3 0 0 1 6 0V9M10 12.5v1.5",
  sleep: "M15.5 12.5A6.5 6.5 0 0 1 7.5 4.5a6.5 6.5 0 1 0 8 8zM12 3h3l-3 3h3",
  restart: "M15.5 8A6 6 0 1 0 16 11M16 4v4h-4",
  star: "M10 2.8l2.2 4.6 5 .6-3.7 3.4 1 5-4.5-2.5-4.5 2.5 1-5L2.8 8l5-.6z",
  link: "M8.5 11.5a3 3 0 0 0 4.2 0l2.6-2.6a3 3 0 0 0-4.2-4.2L10 5.8M11.5 8.5a3 3 0 0 0-4.2 0L4.7 11.1a3 3 0 0 0 4.2 4.2l1.1-1.1",
  calendar: "M3.5 5h13v11.5h-13zM3.5 8.5h13M7 3v3.5M13 3v3.5",
  timer: "M10 5.5a6 6 0 1 1 0 12 6 6 0 0 1 0-12zM10 8.5v3l2 1.5M8 2.5h4",
  search: "M8.5 3.5a5 5 0 1 1 0 10 5 5 0 0 1 0-10zM12.2 12.2L16.5 16.5",
  "chevron-up": "M5 12.5l5-5 5 5",
  "chevron-down": "M5 7.5l5 5 5-5",
  lasso:
    "M10 3.5c4 0 7 2 7 4.5S14 12.5 10 12.5 3 10.5 3 8s3-4.5 7-4.5zM6 11.5c-.5 1.5 0 3 1.5 3.5 1 .3 1.5 1 1 2",
  target:
    "M10 2.5a7.5 7.5 0 1 1 0 15 7.5 7.5 0 0 1 0-15zM10 6.5a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7zM10 10v.01",
  folder: "M3 5.5h5l1.5 1.5H17v9H3z",
  today: "M3.5 5h13v11.5h-13zM3.5 8.5h13M7 3v3.5M13 3v3.5M8 12h4",
  sliders: "M4 6h7M15 6h1M4 14h1M9 14h7M13 4v4M7 12v4",
  logout: "M8 4H4v12h4M12 6.5L15.5 10 12 13.5M15.5 10H8",
  cloud: "M6 15.5h8.5a3.5 3.5 0 0 0 .3-7A5 5 0 0 0 5.2 9.6 3 3 0 0 0 6 15.5z",
  rain: "M6 12.5h8.5a3.5 3.5 0 0 0 .3-7A5 5 0 0 0 5.2 6.6 3 3 0 0 0 6 12.5zM7 15l-1 2.5M10.5 15l-1 2.5M14 15l-1 2.5",
  snow: "M6 12.5h8.5a3.5 3.5 0 0 0 .3-7A5 5 0 0 0 5.2 6.6 3 3 0 0 0 6 12.5zM7 15.5v.01M10 17v.01M13 15.5v.01",
  storm: "M6 12.5h8.5a3.5 3.5 0 0 0 .3-7A5 5 0 0 0 5.2 6.6 3 3 0 0 0 6 12.5zM10.5 13l-2 3h3l-2 3",
  fog: "M4 8h12M3 11h14M5 14h10",
  nightlight: "M14.5 12A5.5 5.5 0 0 1 8 5.5a5.5 5.5 0 1 0 6.5 6.5zM14.5 3v3M13 4.5h3",
  bell: "M10 3a4.5 4.5 0 0 1 4.5 4.5v3l1.5 3H4l1.5-3v-3A4.5 4.5 0 0 1 10 3zM8.3 16a1.8 1.8 0 0 0 3.4 0",
  "bell-off":
    "M3 3l14 14M7.5 5A4 4 0 0 1 14 8.5v3l1.5 2.5H6M5.2 8.2c-.1.1-.2.2-.2.3v3L3.5 14H11M8.5 16.5a1.6 1.6 0 0 0 3 0",
  layers: "M10 3l7 3.5-7 3.5-7-3.5zM3 10l7 3.5 7-3.5M3 13.5L10 17l7-3.5",
  taskbar: "M3 4h14v12H3zM3 13h14M8 14.5h4",
  desktop: "M3 4h14v9H3zM7 16.5h6M10 13v3.5M6 7h2M6 9.5h2",
  shuffle: "M3 6h3l7 8h4M15 12l2 2-2 2M3 14h3l2-2.3M11 8.3L13 6h4M15 4l2 2-2 2",
  gauge: "M3.5 13.5a6.5 6.5 0 1 1 13 0M10 13.5l3-4.5M3.5 13.5h13",
  "mic-off":
    "M3 3l14 14M12.5 9.5v-4a2.5 2.5 0 0 0-4.8-1M7.5 7.5v2a2.5 2.5 0 0 0 4 2M5.5 9.5a4.5 4.5 0 0 0 7.3 3.5M14.4 11a4.5 4.5 0 0 0 .1-1.5M10 14v3",
  brightness:
    "M10 6.5a3.5 3.5 0 1 1 0 7zM10 6.5a3.5 3.5 0 0 0 0 7M10 2v1.5M10 16.5V18M2 10h1.5M16.5 10H18M4.3 4.3l1.1 1.1M14.6 14.6l1.1 1.1M4.3 15.7l1.1-1.1M14.6 5.4l1.1-1.1",
  headphones: "M4 13v-2.5a6 6 0 0 1 12 0V13M4 12h2.5v4.5H4zM13.5 12H16v4.5h-2.5z",
  partly:
    "M8 4.5a3 3 0 0 1 3 1.4M3.5 8.5a3 3 0 0 1 1.8-2.7M7 16h7.5a3 3 0 0 0 .3-6A4.3 4.3 0 0 0 6.5 11 2.5 2.5 0 0 0 7 16z",
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
