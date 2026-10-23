import { useEffect, useMemo, useState } from "react";
import { Icon, type IconName } from "../../components/Icon";
import { formatMinutes } from "../../lib/commands";
import { fromLocalInput, sameDay, toLocalInput, weekFrom } from "../../lib/time";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { Repeat, Task, Weather, WeatherPlace } from "../../services/types";
import { formatCountdown, useNotch } from "../../state/notch";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";

function useClock(ms = 1000): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

const WEATHER_ICON: Record<string, IconName> = {
  Clear: "sun",
  "Mainly clear": "sun",
  "Partly cloudy": "partly",
  Overcast: "cloud",
  Fog: "fog",
  Drizzle: "rain",
  Rain: "rain",
  Snow: "snow",
  Showers: "rain",
  "Snow showers": "snow",
  Thunderstorm: "storm",
};

function WeatherIcon({ label, size }: { label: string; size: number }) {
  return <Icon name={WEATHER_ICON[label] ?? "cloud"} size={size} title={label} />;
}

function WeatherSetup() {
  const update = useSettings((s) => s.update);
  const [q, setQ] = useState("");
  const [places, setPlaces] = useState<WeatherPlace[] | null>(null);
  const [busy, setBusy] = useState(false);
  const search = async () => {
    setBusy(true);
    try {
      setPlaces(await host().weatherSearch(q));
    } catch (e) {
      toast(toWispError(e).message, "warning");
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="card weather weather-setup">
      <p className="card-title">Weather</p>
      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          void search();
        }}
      >
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Your town or city"
          aria-label="Town or city for weather"
        />
        <button className="btn btn-small" disabled={busy || q.trim().length < 2}>
          Find
        </button>
      </form>
      {places && (
        <ul className="pick-list">
          {places.length === 0 && <li className="muted">No matches</li>}
          {places.map((p) => (
            <li key={`${p.latitude},${p.longitude}`}>
              <button className="link-btn" onClick={() => void update({ weatherPlace: p })}>
                {p.name}
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="fine">Uses Open-Meteo (no account). Only the place you pick is sent.</p>
    </div>
  );
}

function WeatherCard() {
  const place = useSettings((s) => s.snapshot?.settings.weatherPlace ?? null);
  const [w, setW] = useState<Weather | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!place) return;
    let alive = true;
    host()
      .weatherGet()
      .then((x) => alive && setW(x))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [place]);
  if (!place) return <WeatherSetup />;
  return (
    <div className="card weather">
      <p className="card-title">
        {place.name.split(",")[0]}
        <button
          className="icon-btn icon-btn-tiny"
          title="Change place"
          aria-label="Change weather place"
          onClick={() => void useSettings.getState().update({ weatherPlace: null })}
        >
          <Icon name="close" size={12} />
        </button>
      </p>
      {!w ? (
        <p className="muted">{failed ? "Weather unavailable right now." : "Loading…"}</p>
      ) : (
        <>
          <div className="weather-now">
            <span className="weather-icon">
              <WeatherIcon label={w.label} size={30} />
            </span>
            <span className="weather-temp">{Math.round(w.temperature)}°</span>
            <span className="weather-label">
              {w.label}
              <span className="muted"> · wind {Math.round(w.windKmh)} km/h</span>
            </span>
          </div>
          <ol className="weather-days">
            {w.days.map((d) => (
              <li key={d.date} title={d.label}>
                <span className="muted">
                  {new Date(`${d.date}T12:00:00`).toLocaleDateString([], { weekday: "short" })}
                </span>
                <WeatherIcon label={d.label} size={16} />
                <span>
                  {Math.round(d.max)}°<span className="muted">/{Math.round(d.min)}°</span>
                </span>
              </li>
            ))}
          </ol>
        </>
      )}
    </div>
  );
}

function FocusCard() {
  const timer = useNotch((n) => n.timer);
  const start = useNotch((n) => n.startTimer);
  const stop = useNotch((n) => n.stopTimer);
  const def = useSettings((s) => s.snapshot?.settings.focusMinutes ?? 25);
  const now = useClock(1000).getTime();
  const total = timer ? timer.endsAt - timer.startedAt : 1;
  const left = timer ? Math.max(0, timer.endsAt - now) : 0;
  const frac = timer ? 1 - left / total : 0;
  const R = 34;
  const C = 2 * Math.PI * R;
  return (
    <div className="card focus">
      <p className="card-title">Focus</p>
      <div className="focus-body">
        <svg className="focus-ring" viewBox="0 0 80 80" width="80" height="80" aria-hidden>
          <circle cx="40" cy="40" r={R} className="ring-bg" />
          {timer && (
            <circle
              cx="40"
              cy="40"
              r={R}
              className="ring-fg"
              strokeDasharray={`${C * frac} ${C}`}
              transform="rotate(-90 40 40)"
            />
          )}
        </svg>
        <div className="focus-side">
          <span className="focus-time" aria-live="off">
            {timer ? formatCountdown(left) : formatMinutes(def)}
          </span>
          {timer ? (
            <button className="btn btn-small" onClick={() => void stop()}>
              <Icon name="stop" size={12} /> Stop
            </button>
          ) : (
            <div className="row-wrap">
              {[15, def, 50]
                .filter((v, i, a) => a.indexOf(v) === i)
                .map((m) => (
                  <button key={m} className="btn btn-small" onClick={() => void start(m)}>
                    {m} min
                  </button>
                ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

const REPEAT_LABEL: Record<Repeat, string> = {
  none: "Once",
  daily: "Every day",
  weekdays: "Weekdays",
  weekly: "Every week",
};

function Reminders({ day }: { day: Date | null }) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [title, setTitle] = useState("");
  const [due, setDue] = useState(() => toLocalInput(new Date(Date.now() + 3600_000)));
  const [repeat, setRepeat] = useState<Repeat>("none");
  useEffect(() => {
    const load = () =>
      void host()
        .tasksList(false)
        .then(setTasks)
        .catch(() => {});
    load();
    return host().onDataChanged((s) => (s === "tasks" || s === "everything") && load());
  }, []);
  const shown = day
    ? tasks.filter((t) => t.dueAt && sameDay(new Date(t.dueAt), day))
    : tasks.slice(0, 6);
  const add = async () => {
    try {
      await host().taskCreate({
        title,
        dueAt: fromLocalInput(due),
        notificationEnabled: true,
        repeat,
      });
      setTitle("");
      setRepeat("none");
      toast("Reminder added", "success");
    } catch (e) {
      toast(toWispError(e).message, "warning");
    }
  };
  return (
    <div className="card reminders-card">
      <p className="card-title">
        {day
          ? day.toLocaleDateString([], { weekday: "long", day: "numeric", month: "short" })
          : "Up next"}
      </p>
      {shown.length === 0 && <p className="muted">Nothing scheduled.</p>}
      <ul className="todo">
        {shown.map((t) => (
          <li key={t.id}>
            <button
              className="todo-check"
              aria-label={`Mark "${t.title}" done`}
              onClick={() =>
                void host()
                  .taskComplete(t.id, true)
                  .then(() => toast("Done", "success"))
              }
            />
            <span className="todo-title">{t.title}</span>
            <span className="todo-when muted">
              {t.repeat !== "none" && <Icon name="restart" size={11} />}
              {t.dueAt
                ? new Date(t.dueAt).toLocaleString([], {
                    weekday: day ? undefined : "short",
                    hour: "2-digit",
                    minute: "2-digit",
                  })
                : "No time"}
            </span>
          </li>
        ))}
      </ul>
      <form
        className="todo-add"
        onSubmit={(e) => {
          e.preventDefault();
          if (title.trim()) void add();
        }}
      >
        <input
          className="input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Add a reminder"
          aria-label="New reminder"
          maxLength={200}
        />
        <input
          className="input input-time"
          type="datetime-local"
          value={due}
          onChange={(e) => setDue(e.target.value)}
          aria-label="When"
        />
        <select
          className="input"
          value={repeat}
          onChange={(e) => setRepeat(e.target.value as Repeat)}
          aria-label="Repeat"
        >
          {(Object.keys(REPEAT_LABEL) as Repeat[]).map((r) => (
            <option key={r} value={r}>
              {REPEAT_LABEL[r]}
            </option>
          ))}
        </select>
        <button className="btn btn-small btn-primary" disabled={!title.trim()}>
          Add
        </button>
      </form>
    </div>
  );
}

export default function TodayTab() {
  const now = useClock(15_000);
  const [day, setDay] = useState<Date | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  useEffect(() => {
    const load = () =>
      void host()
        .tasksList(false)
        .then(setTasks)
        .catch(() => {});
    load();
    return host().onDataChanged((s) => (s === "tasks" || s === "everything") && load());
  }, []);
  const week = useMemo(() => weekFrom(now), [now]);
  return (
    <div className="today">
      <div className="today-top">
        <div className="clock">
          <span className="clock-time">
            {now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </span>
          <span className="clock-date">
            {now.toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" })}
          </span>
        </div>
        <ol className="week" aria-label="This week">
          {week.map((d) => {
            const count = tasks.filter((t) => t.dueAt && sameDay(new Date(t.dueAt), d)).length;
            const active = day ? sameDay(d, day) : false;
            return (
              <li key={d.toISOString()}>
                <button
                  className={`week-day ${active ? "active" : ""} ${sameDay(d, now) ? "today" : ""}`}
                  onClick={() => setDay(active ? null : d)}
                  aria-pressed={active}
                  aria-label={`${d.toLocaleDateString([], { weekday: "long", day: "numeric" })}: ${count} reminders`}
                >
                  <span>{d.toLocaleDateString([], { weekday: "narrow" })}</span>
                  <strong>{d.getDate()}</strong>
                  <span className="week-dots" aria-hidden>
                    {"•".repeat(Math.min(count, 3))}
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
      </div>
      <div className="today-grid">
        <Reminders day={day} />
        <div className="today-side">
          <FocusCard />
          <WeatherCard />
        </div>
      </div>
    </div>
  );
}
