import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { WispMark } from "../../components/WispMark";
import { useOrbState } from "../../hooks/useOrbState";
import { host } from "../../services/host";
import type { AgentJob, NowPlaying } from "../../services/types";
import { useDueReminders } from "../../state/assist";
import { formatCountdown, useNotch } from "../../state/notch";
import { ORB_LABELS } from "../../state/orb";

function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

/** Running agents, for the progress dot. */
function useRunningJobs(): AgentJob[] {
  const [jobs, setJobs] = useState<AgentJob[]>([]);
  useEffect(() => {
    const load = () =>
      void host()
        .agentsList()
        .then((j) => setJobs(j.filter((x) => x.status === "running")))
        .catch(() => {});
    load();
    return host().onDataChanged((scope) => (scope === "jobs" || scope === "everything") && load());
  }, []);
  return jobs;
}

/** What the hover notch shows: the most relevant live thing, else the time. */
export function Peek() {
  const state = useOrbState();
  const now = useNow(1000);
  const timer = useNotch((n) => n.timer);
  const jobs = useRunningJobs();
  const reminder = useDueReminders((r) => r.due[0]);
  const [playing, setPlaying] = useState<NowPlaying | null>(null);

  useEffect(() => {
    let alive = true;
    void host()
      .nowPlaying()
      .then((p) => alive && setPlaying(p?.playing ? p : null))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  let main: React.ReactNode;
  if (state === "thinking") main = <span>Thinking…</span>;
  else if (reminder)
    main = (
      <span className="peek-reminder">
        <Icon name="bell" size={14} />
        <span className="peek-ellipsis">{reminder.title}</span>
      </span>
    );
  else if (timer)
    main = (
      <span className="peek-timer">
        <Icon name="timer" size={14} /> Focus {formatCountdown(timer.endsAt - now)}
      </span>
    );
  else if (playing)
    main = (
      <span className="peek-media">
        <Icon name="play" size={12} />
        <span className="peek-ellipsis">
          {playing.title}
          {playing.artist ? ` · ${playing.artist}` : ""}
        </span>
      </span>
    );
  else
    main = (
      <span className="peek-clock">
        {new Date(now).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
        <span className="muted">
          {" "}
          ·{" "}
          {new Date(now).toLocaleDateString([], {
            weekday: "short",
            day: "numeric",
            month: "short",
          })}
        </span>
      </span>
    );

  return (
    <span className="peek">
      <WispMark state={state} size={20} />
      <span className="peek-main" aria-live="polite">
        {main}
      </span>
      {jobs.length > 0 && (
        <span
          className="peek-dot"
          title={`${jobs.length} agent${jobs.length > 1 ? "s" : ""} running`}
        >
          <span className="sr-only">{jobs.length} agents running</span>
        </span>
      )}
      <span className="sr-only">{ORB_LABELS[state]}</span>
    </span>
  );
}
