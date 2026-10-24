import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { AgentJob, AgentKind } from "../../services/types";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { timeAgo } from "./util";

function useJobs(): AgentJob[] {
  const [jobs, setJobs] = useState<AgentJob[]>([]);
  useEffect(() => {
    const load = () =>
      void host()
        .agentsList()
        .then(setJobs)
        .catch(() => {});
    load();
    return host().onDataChanged((s) => (s === "jobs" || s === "everything") && load());
  }, []);
  return jobs;
}

async function start(kind: AgentKind) {
  try {
    await host().agentStart(kind);
  } catch (e) {
    toast(toWispError(e).message, "warning", 3500);
  }
}

export default function AgentsTab() {
  const jobs = useJobs();
  const [topic, setTopic] = useState("");
  const screenOn = useSettings((s) => s.snapshot?.settings.screenContextEnabled ?? true);
  return (
    <div className="agents">
      <div className="agent-cards">
        <form
          className="card agent-card"
          onSubmit={(e) => {
            e.preventDefault();
            if (topic.trim().length < 3) return;
            void start({ kind: "research", topic });
            setTopic("");
          }}
        >
          <p className="card-title">
            <Icon name="globe" size={15} /> Research brief
          </p>
          <p className="fine">
            Searches the web with Gemini and writes a sourced Markdown brief to Documents\Wisp.
          </p>
          <div className="row">
            <input
              className="input"
              value={topic}
              onChange={(e) => setTopic(e.target.value)}
              placeholder="e.g. best budget keyboards"
              aria-label="Research topic"
              maxLength={500}
            />
            <button className="btn btn-small btn-primary" disabled={topic.trim().length < 3}>
              Start
            </button>
          </div>
        </form>
        <div className="card agent-card">
          <p className="card-title">
            <Icon name="screen" size={15} /> Screen replica
          </p>
          <p className="fine">
            Takes one screenshot of this screen (Wisp hides first), sends it to Gemini and saves a
            standalone HTML copy of the layout to Documents\Wisp.
          </p>
          <button
            className="btn btn-small"
            disabled={!screenOn}
            title={screenOn ? undefined : "Turn on screen features in Settings → Privacy"}
            onClick={() => void start({ kind: "replica" })}
          >
            Recreate this screen
          </button>
        </div>
      </div>
      {jobs.length === 0 ? (
        <p className="muted pad">Agents you start run in the background and show up here.</p>
      ) : (
        <ul className="jobs">
          {jobs.map((j) => (
            <li key={j.id} className={`job job-${j.status}`}>
              <div className="job-main">
                <strong>{j.title}</strong>
                <span className="muted">
                  {j.status === "running"
                    ? (j.detail ?? "Working…")
                    : j.status === "done"
                      ? `Saved · ${timeAgo(j.finishedAt ?? j.createdAt)}`
                      : (j.detail ?? j.status)}
                </span>
                {j.status === "running" && (
                  <div
                    className="meter"
                    role="progressbar"
                    aria-label="Progress"
                    aria-valuenow={Math.round(j.progress * 100)}
                    aria-valuemin={0}
                    aria-valuemax={100}
                  >
                    <span style={{ width: `${Math.max(6, j.progress * 100)}%` }} />
                  </div>
                )}
              </div>
              <div className="job-actions">
                {j.status === "running" && (
                  <button className="btn btn-small" onClick={() => void host().agentCancel(j.id)}>
                    Cancel
                  </button>
                )}
                {j.status === "done" && (
                  <button
                    className="btn btn-small btn-primary"
                    onClick={() =>
                      void host()
                        .agentOpen(j.id)
                        .catch((e) => toast(toWispError(e).message, "warning"))
                    }
                  >
                    Open
                  </button>
                )}
                {j.status !== "running" && (
                  <button
                    className="icon-btn icon-btn-tiny"
                    aria-label="Remove from list"
                    onClick={() => void host().agentDelete(j.id)}
                  >
                    <Icon name="close" size={13} />
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
