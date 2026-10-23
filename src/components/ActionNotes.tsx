import { useConversation, type ActionNote } from "../state/conversation";
import { Icon } from "./Icon";

const LABEL: Record<ActionNote["status"], string> = {
  pending: "Working…",
  done: "Done",
  scheduled: "Scheduled",
  declined: "Not done",
  rejected: "Blocked",
  failed: "Failed",
};

/** Model-proposed actions: confirmation prompts and their outcomes. */
export function ActionNotes({ notes }: { notes: ActionNote[] }) {
  const respond = useConversation((s) => s.respondAction);
  return (
    <div className="actions">
      {notes.map((n, i) =>
        n.confirmId ? (
          <div
            key={i}
            className="action-confirm"
            role="group"
            aria-label="Wisp is asking for permission"
          >
            <p>
              <Icon name="info" size={16} /> Wisp wants to: <strong>{n.description}</strong>
            </p>
            <div className="row-start">
              <button
                className="btn btn-primary btn-small"
                onClick={() => respond(n.confirmId!, true)}
              >
                Allow
              </button>
              <button className="btn btn-small" onClick={() => respond(n.confirmId!, false)}>
                Don't allow
              </button>
            </div>
          </div>
        ) : (
          <p key={i} className={`action-note action-${n.status}`}>
            <Icon
              name={
                n.status === "done"
                  ? "check"
                  : n.status === "scheduled"
                    ? "timer"
                    : n.status === "pending"
                      ? "info"
                      : "warning"
              }
              size={14}
            />
            <span>
              <strong>{LABEL[n.status]}:</strong> {n.message || n.description}
            </span>
          </p>
        ),
      )}
    </div>
  );
}
