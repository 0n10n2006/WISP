import { useCallback, useEffect, useState } from "react";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { ActionAudit, DeleteScope, RequestLog, StorageStats } from "../../services/types";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { StatusLine, useStatus } from "./common";

const fmtTime = (iso: string) =>
  new Date(iso).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

const STATUS_LABEL: Record<string, string> = {
  ok: "Answered",
  error: "Failed",
  cancelled: "Stopped",
  done: "Done",
  rejected: "Rejected",
  declined: "Declined",
  failed: "Failed",
};

const KIND_LABEL: Record<string, string> = {
  text: "Question",
  screen: "Screen question",
  clipboard: "Clipboard",
  voice: "Voice",
  dictation: "Dictation",
};

function useDataReload(load: () => void) {
  useEffect(() => {
    load();
    return host().onDataChanged(() => load());
  }, [load]);
}

export function HistoryTab() {
  const contentHistory = useSettings((s) => s.snapshot?.settings.contentHistory ?? false);
  const [requests, setRequests] = useState<RequestLog[] | null>(null);
  const [actions, setActions] = useState<ActionAudit[]>([]);
  const [confirm, setConfirm] = useState(false);
  const { status, fail } = useStatus();

  const load = useCallback(() => {
    Promise.all([host().historyList(200), host().actionsList(100)])
      .then(([r, a]) => {
        setRequests(r);
        setActions(a);
      })
      .catch((e) => fail(toWispError(e)));
  }, [fail]);
  useDataReload(load);

  return (
    <div className="tab tab-wide">
      <h2>History</h2>
      <p className="setting-desc">
        {contentHistory
          ? "Wisp keeps the time, type, status and text of recent requests on this PC."
          : "Wisp keeps only the time, type and status of recent requests. Conversation text isn't stored. You can change this in Privacy."}
      </p>
      <StatusLine status={status} />
      <section className="group">
        <h3>Requests</h3>
        {requests === null ? (
          <p className="muted">Loading…</p>
        ) : requests.length === 0 ? (
          <p className="muted">No requests yet.</p>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th scope="col">When</th>
                <th scope="col">Type</th>
                <th scope="col">Result</th>
                <th scope="col">Model</th>
                <th scope="col" className="num">
                  Time
                </th>
              </tr>
            </thead>
            <tbody>
              {requests.map((r) => (
                <tr key={r.id}>
                  <td>{fmtTime(r.timestamp)}</td>
                  <td>
                    {KIND_LABEL[r.contextKind] ?? r.contextKind}
                    {r.prompt && <div className="cell-sub">{r.prompt.slice(0, 140)}</div>}
                  </td>
                  <td>
                    <span className={`pill pill-${r.status}`}>
                      {r.status === "ok" ? (
                        <Icon name="check" size={12} />
                      ) : r.status === "error" ? (
                        <Icon name="warning" size={12} />
                      ) : null}
                      {STATUS_LABEL[r.status] ?? r.status}
                    </span>
                    {r.errorCode && r.status === "error" && (
                      <div className="cell-sub">{r.errorCode.replace(/_/g, " ")}</div>
                    )}
                  </td>
                  <td className="cell-mono">{r.model}</td>
                  <td className="num">{(r.latencyMs / 1000).toFixed(1)} s</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <section className="group">
        <h3>Desktop actions</h3>
        {actions.length === 0 ? (
          <p className="muted">No actions yet.</p>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th scope="col">When</th>
                <th scope="col">Action</th>
                <th scope="col">Details</th>
                <th scope="col">Result</th>
              </tr>
            </thead>
            <tbody>
              {actions.map((a) => (
                <tr key={a.id}>
                  <td>{fmtTime(a.timestamp)}</td>
                  <td>{a.actionType.replace(/_/g, " ")}</td>
                  <td className="cell-mono">{a.summary}</td>
                  <td>
                    <span className={`pill pill-${a.outcome}`}>
                      {STATUS_LABEL[a.outcome] ?? a.outcome}
                    </span>
                    {a.userConfirmed && <div className="cell-sub">confirmed by you</div>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <div className="row-start">
        <button
          className="btn btn-danger-ghost"
          disabled={!requests?.length && !actions.length}
          onClick={() => setConfirm(true)}
        >
          <Icon name="trash" size={16} /> Clear history
        </button>
      </div>
      {confirm && (
        <ConfirmDialog
          title="Clear history?"
          confirmLabel="Clear history"
          destructive
          onCancel={() => setConfirm(false)}
          onConfirm={async () => {
            setConfirm(false);
            try {
              await host().dataDelete("history");
              toast("History cleared", "success");
            } catch (e) {
              fail(toWispError(e));
            }
          }}
        >
          <p>
            This removes request and action history from this PC. It doesn't affect anything Google
            already received.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}

const DELETE_COPY: Record<DeleteScope, { label: string; title: string; body: string }> = {
  history: {
    label: "Delete history",
    title: "Delete history?",
    body: "Request and action history will be removed from this PC.",
  },
  memory: {
    label: "Delete all memory",
    title: "Delete all memory?",
    body: "Everything you've asked Wisp to remember will be removed.",
  },
  clipboard: {
    label: "Delete clipboard history",
    title: "Delete clipboard history?",
    body: "Everything in Wisp's clipboard history, including pinned items, will be removed.",
  },
  tasks: {
    label: "Delete all reminders",
    title: "Delete all reminders?",
    body: "All reminders, including upcoming ones, will be removed.",
  },
  settings: {
    label: "Reset settings",
    title: "Reset settings?",
    body: "Theme, shortcuts, window and privacy settings return to their defaults. Your API key stays.",
  },
  everything: {
    label: "Delete everything",
    title: "Delete all Wisp data?",
    body: "History, memory, reminders, clipboard history, crash logs, backups and settings will be removed, and your API key will be deleted from Windows Credential Manager. Files agents saved in Documents\\Wisp are kept. Windows will ask you to confirm once more, then Wisp starts setup again.",
  },
};

export function DataTab() {
  const snapshot = useSettings((s) => s.snapshot);
  const [stats, setStats] = useState<StorageStats | null>(null);
  const [pending, setPending] = useState<DeleteScope | null>(null);
  const [report, setReport] = useState<string | null>(null);
  const [confirmExport, setConfirmExport] = useState(false);
  const { status, ok, fail } = useStatus();

  const load = useCallback(() => {
    host()
      .dataStats()
      .then(setStats)
      .catch((e) => fail(toWispError(e)));
  }, [fail]);
  useDataReload(load);

  const rows: { scope: DeleteScope; what: string; count?: number }[] = [
    {
      scope: "history",
      what: "Request & action history",
      count: stats ? stats.requests + stats.actions : undefined,
    },
    { scope: "memory", what: "Memory", count: stats?.memories },
    { scope: "tasks", what: "Reminders", count: stats?.tasks },
    { scope: "clipboard", what: "Clipboard history", count: stats?.clips },
    { scope: "settings", what: "Settings" },
  ];

  return (
    <div className="tab">
      <h2>Data & diagnostics</h2>
      {snapshot?.warnings.map((w) => (
        <div key={w} className="banner banner-warning" role="alert">
          <div className="banner-body">
            <strong>Notice</strong>
            <p>{w}</p>
          </div>
        </div>
      ))}
      <section className="group">
        <h3>Stored on this PC</h3>
        <p className="setting-desc">
          Deleting here removes data from this PC only. It can't erase prompts Google already
          received; Google's own retention rules apply to those.
        </p>
        {rows.map((r) => (
          <div className="setting-row" key={r.scope}>
            <div className="setting-text">
              <span className="row-label">{r.what}</span>
              {r.count !== undefined && (
                <p className="setting-desc">
                  {r.count} item{r.count === 1 ? "" : "s"}
                </p>
              )}
            </div>
            <button className="btn btn-danger-ghost" onClick={() => setPending(r.scope)}>
              {DELETE_COPY[r.scope].label}
            </button>
          </div>
        ))}
        <div className="row-start">
          <button className="btn btn-danger" onClick={() => setPending("everything")}>
            <Icon name="trash" size={16} /> Delete everything
          </button>
        </div>
      </section>

      <section className="group">
        <h3>Diagnostics</h3>
        <p className="setting-desc">
          A diagnostics file helps troubleshoot problems. It contains app and OS versions, your
          settings (without app paths or window position), storage counts, and the status of recent
          requests. It never contains your API key, prompts, answers, memory or reminder text.
        </p>
        <div className="row-start">
          <button
            className="btn"
            onClick={async () => {
              try {
                setReport(await host().diagnosticsReport());
              } catch (e) {
                fail(toWispError(e));
              }
            }}
          >
            Preview diagnostics
          </button>
          {report && (
            <>
              <button
                className="btn"
                onClick={async () => {
                  await host().copyText(report);
                  toast("Copied", "success");
                }}
              >
                <Icon name="copy" size={16} /> Copy
              </button>
              <button className="btn btn-primary" onClick={() => setConfirmExport(true)}>
                Save to Downloads
              </button>
            </>
          )}
        </div>
        {report && (
          <pre className="report" tabIndex={0} aria-label="Diagnostics preview">
            {report}
          </pre>
        )}
        <StatusLine status={status} />
      </section>

      {pending && (
        <ConfirmDialog
          title={DELETE_COPY[pending].title}
          confirmLabel={DELETE_COPY[pending].label}
          destructive
          onCancel={() => setPending(null)}
          onConfirm={async () => {
            const scope = pending;
            setPending(null);
            try {
              setStats(await host().dataDelete(scope));
              ok("Deleted.");
            } catch (e) {
              const err = toWispError(e);
              // Declining Windows' own confirmation isn't an error.
              if (err.code === "cancelled") ok("Nothing was deleted.");
              else fail(err);
            }
          }}
        >
          <p>{DELETE_COPY[pending].body}</p>
          <p>This can't be undone.</p>
        </ConfirmDialog>
      )}
      {confirmExport && (
        <ConfirmDialog
          title="Save diagnostics file?"
          confirmLabel="Save file"
          onCancel={() => setConfirmExport(false)}
          onConfirm={async () => {
            setConfirmExport(false);
            try {
              ok(`Saved to ${await host().diagnosticsSave()}`);
            } catch (e) {
              fail(toWispError(e));
            }
          }}
        >
          <p>
            The file shown in the preview will be saved to your Downloads folder. Review it before
            sharing it with anyone.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}
