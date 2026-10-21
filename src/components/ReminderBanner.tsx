import { host } from "../services/host";
import { useDueReminders } from "../state/assist";
import { toast } from "../state/toast";
import { Icon } from "./Icon";

export function ReminderBanner() {
  const { due, dismiss } = useDueReminders();
  if (!due.length) return null;
  return (
    <div className="reminders-due">
      {due.map((t) => (
        <div key={t.id} className="banner banner-reminder" role="alert">
          <span className="banner-icon">
            <Icon name="info" size={18} />
          </span>
          <div className="banner-body">
            <strong>Reminder: {t.title}</strong>
            {t.notes && <p>{t.notes}</p>}
            <div className="banner-actions">
              <button
                className="btn btn-small"
                onClick={() => {
                  dismiss(t.id);
                  void host()
                    .taskComplete(t.id, true)
                    .then(() => toast("Marked done", "success"));
                }}
              >
                <Icon name="check" size={14} /> Done
              </button>
              <button
                className="btn btn-small btn-ghost"
                onClick={() => {
                  dismiss(t.id);
                  void host()
                    .taskSnooze(t.id, new Date(Date.now() + 10 * 60_000).toISOString())
                    .then(() => toast("Snoozed for 10 minutes"));
                }}
              >
                Snooze 10 min
              </button>
            </div>
          </div>
          <button
            className="icon-btn banner-close"
            onClick={() => dismiss(t.id)}
            aria-label="Dismiss reminder"
          >
            <Icon name="close" size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
