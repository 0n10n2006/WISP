import { useCallback, useEffect, useState, type FormEvent } from "react";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type {
  AllowedApp,
  InstalledApp,
  MemoryCategory,
  MemoryEntry,
  NewTask,
  Task,
} from "../../services/types";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { Choice, StatusLine, Toggle, useStatus } from "./common";

/** `datetime-local` value ↔ ISO string helpers (local time). */
export { fromLocalInput, toLocalInput } from "../../lib/time";
import { fromLocalInput, toLocalInput } from "../../lib/time";

const fmtDue = (iso: string | null) =>
  iso
    ? new Date(iso).toLocaleString(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "No time set";

function useReload(scope: string, load: () => void) {
  useEffect(() => {
    load();
    return host().onDataChanged((s) => {
      if (s === scope || s === "everything") load();
    });
  }, [scope, load]);
}

// ---------------------------------------------------------------------------

function ReminderEditor({
  task,
  onSave,
  onCancel,
}: {
  task: Task;
  onSave(next: NewTask): void;
  onCancel(): void;
}) {
  const [title, setTitle] = useState(task.title);
  const [due, setDue] = useState(task.dueAt ? toLocalInput(new Date(task.dueAt)) : "");
  const [notes, setNotes] = useState(task.notes ?? "");
  return (
    <form
      className="item-main reminder-edit"
      aria-label={`Edit "${task.title}"`}
      onSubmit={(e) => {
        e.preventDefault();
        onSave({
          title,
          notes: notes || null,
          dueAt: fromLocalInput(due),
          notificationEnabled: task.notificationEnabled,
        });
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onCancel();
        }
      }}
    >
      <label className="sr-only" htmlFor={`edit-title-${task.id}`}>
        Title
      </label>
      <input
        id={`edit-title-${task.id}`}
        type="text"
        value={title}
        maxLength={200}
        onChange={(e) => setTitle(e.target.value)}
        autoFocus
      />
      <label className="sr-only" htmlFor={`edit-due-${task.id}`}>
        When
      </label>
      <input
        id={`edit-due-${task.id}`}
        type="datetime-local"
        value={due}
        onChange={(e) => setDue(e.target.value)}
      />
      <label className="sr-only" htmlFor={`edit-notes-${task.id}`}>
        Notes
      </label>
      <input
        id={`edit-notes-${task.id}`}
        type="text"
        value={notes}
        placeholder="Notes"
        maxLength={2000}
        onChange={(e) => setNotes(e.target.value)}
      />
      <div className="row-start">
        <button className="btn btn-small btn-primary" disabled={!title.trim()}>
          Save
        </button>
        <button type="button" className="btn btn-small btn-ghost" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

export function RemindersTab() {
  const notifications = useSettings((s) => s.snapshot?.settings.notificationsEnabled ?? true);
  const update = useSettings((s) => s.update);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [showDone, setShowDone] = useState(false);
  const [title, setTitle] = useState("");
  const [due, setDue] = useState(() => toLocalInput(new Date(Date.now() + 3600_000)));
  const [notes, setNotes] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<Task | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [editing, setEditing] = useState<string | null>(null);
  const { status, fail, clear } = useStatus();

  const load = useCallback(() => {
    host()
      .tasksList(showDone)
      .then((t) => {
        setTasks(t);
        setNow(Date.now());
      })
      .catch((e) => fail(toWispError(e)));
  }, [showDone, fail]);
  useReload("tasks", load);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    clear();
    try {
      await host().taskCreate({
        title,
        notes: notes || null,
        dueAt: fromLocalInput(due),
        notificationEnabled: true,
      });
      setTitle("");
      setNotes("");
      toast("Reminder added", "success");
    } catch (err) {
      fail(toWispError(err));
    }
  };

  const act = async (fn: () => Promise<unknown>) => {
    clear();
    try {
      await fn();
    } catch (err) {
      fail(toWispError(err));
    }
  };

  const snooze = (t: Task, minutes: number) =>
    act(() => host().taskSnooze(t.id, new Date(Date.now() + minutes * 60_000).toISOString()));

  return (
    <div className="tab">
      <h2>Reminders</h2>
      <p className="setting-desc">
        Reminders are stored on this PC and work offline. You can also ask Wisp, e.g. "remind me to
        call Sam at 4pm".
      </p>

      <form className="group reminder-form" onSubmit={add}>
        <h3>New reminder</h3>
        <label htmlFor="rem-title">Title</label>
        <input
          id="rem-title"
          type="text"
          placeholder="e.g. Call Sam"
          value={title}
          maxLength={200}
          onChange={(e) => setTitle(e.target.value)}
          required
        />
        <div className="form-grid">
          <div>
            <label htmlFor="rem-due">When</label>
            <input
              id="rem-due"
              type="datetime-local"
              value={due}
              onChange={(e) => setDue(e.target.value)}
            />
          </div>
          <div>
            <label htmlFor="rem-notes">Notes (optional)</label>
            <input
              id="rem-notes"
              type="text"
              value={notes}
              maxLength={2000}
              onChange={(e) => setNotes(e.target.value)}
            />
          </div>
        </div>
        <div className="row-start">
          <button className="btn btn-primary" disabled={!title.trim()}>
            <Icon name="plus" size={16} /> Add reminder
          </button>
        </div>
        <StatusLine status={status} />
      </form>

      <section className="group">
        <div className="row-between heading-row">
          <h3>{showDone ? "All reminders" : "Upcoming"}</h3>
          <label className="inline-check">
            <input
              type="checkbox"
              checked={showDone}
              onChange={(e) => setShowDone(e.target.checked)}
            />{" "}
            Show completed
          </label>
        </div>
        {tasks.length === 0 ? (
          <p className="muted">No reminders.</p>
        ) : (
          <ul className="item-list" aria-label="Reminders">
            {tasks.map((t) => {
              const overdue = !t.completedAt && t.dueAt && new Date(t.dueAt).getTime() < now;
              return (
                <li key={t.id} className={`item ${t.completedAt ? "item-done" : ""}`}>
                  <input
                    type="checkbox"
                    aria-label={`Mark "${t.title}" ${t.completedAt ? "not done" : "done"}`}
                    checked={!!t.completedAt}
                    onChange={(e) => void act(() => host().taskComplete(t.id, e.target.checked))}
                  />
                  {editing === t.id ? (
                    <ReminderEditor
                      task={t}
                      onCancel={() => setEditing(null)}
                      onSave={async (next) => {
                        clear();
                        try {
                          await host().taskUpdate(t.id, next);
                          setEditing(null);
                          toast("Reminder updated", "success");
                        } catch (err) {
                          fail(toWispError(err));
                        }
                      }}
                    />
                  ) : (
                    <div className="item-main">
                      <span className="item-title">{t.title}</span>
                      <span className={`item-sub ${overdue ? "overdue" : ""}`}>
                        {overdue && <Icon name="warning" size={12} />} {fmtDue(t.dueAt)}
                        {t.notes && ` · ${t.notes}`}
                      </span>
                    </div>
                  )}
                  {!t.completedAt && editing !== t.id && (
                    <button className="btn btn-small btn-ghost" onClick={() => setEditing(t.id)}>
                      Edit
                    </button>
                  )}
                  {!t.completedAt && editing !== t.id && (
                    <div className="item-actions">
                      <button
                        className="btn btn-small btn-ghost"
                        onClick={() => void snooze(t, 10)}
                      >
                        +10 min
                      </button>
                      <button
                        className="btn btn-small btn-ghost"
                        onClick={() => void snooze(t, 60)}
                      >
                        +1 h
                      </button>
                    </div>
                  )}
                  <button
                    className="icon-btn"
                    aria-label={`Delete "${t.title}"`}
                    onClick={() => setConfirmDelete(t)}
                  >
                    <Icon name="trash" size={16} />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section className="group">
        <Toggle
          id="notifications"
          label="Show Windows notifications"
          description="When off, due reminders only appear inside Wisp."
          checked={notifications}
          onChange={(notificationsEnabled) => void update({ notificationsEnabled })}
        />
      </section>

      {confirmDelete && (
        <ConfirmDialog
          title="Delete reminder?"
          confirmLabel="Delete"
          destructive
          onCancel={() => setConfirmDelete(null)}
          onConfirm={() => {
            const t = confirmDelete;
            setConfirmDelete(null);
            void act(() => host().taskDelete(t.id));
          }}
        >
          <p>"{confirmDelete.title}" will be removed.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------

const CATEGORIES: { value: MemoryCategory; label: string }[] = [
  { value: "preference", label: "Preference" },
  { value: "fact", label: "Fact" },
  { value: "project", label: "Project" },
  { value: "people", label: "People" },
  { value: "other", label: "Other" },
];

function MemoryRow({ m, onError }: { m: MemoryEntry; onError(e: unknown): void }) {
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState(m.content);
  const [category, setCategory] = useState<MemoryCategory>(m.category);
  const [confirm, setConfirm] = useState(false);

  const save = async () => {
    try {
      await host().memoryUpdate(m.id, content, category, m.enabled);
      setEditing(false);
    } catch (e) {
      onError(e);
    }
  };

  return (
    <li className={`item ${m.enabled ? "" : "item-done"}`}>
      <input
        type="checkbox"
        role="switch"
        className="switch switch-small"
        aria-label={`Use "${m.content}" in requests`}
        checked={m.enabled}
        onChange={(e) =>
          void host().memoryUpdate(m.id, m.content, m.category, e.target.checked).catch(onError)
        }
      />
      <div className="item-main">
        {editing ? (
          <div className="memory-edit">
            <label className="sr-only" htmlFor={`mem-${m.id}`}>
              Memory text
            </label>
            <input
              id={`mem-${m.id}`}
              type="text"
              value={content}
              maxLength={500}
              onChange={(e) => setContent(e.target.value)}
            />
            <select
              aria-label="Category"
              className="select-inline"
              value={category}
              onChange={(e) => setCategory(e.target.value as MemoryCategory)}
            >
              {CATEGORIES.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
            <button className="btn btn-small btn-primary" onClick={() => void save()}>
              Save
            </button>
            <button
              className="btn btn-small btn-ghost"
              onClick={() => {
                setEditing(false);
                setContent(m.content);
              }}
            >
              Cancel
            </button>
          </div>
        ) : (
          <>
            <span className="item-title">{m.content}</span>
            <span className="item-sub">
              {CATEGORIES.find((c) => c.value === m.category)?.label ?? m.category}
            </span>
          </>
        )}
      </div>
      {!editing && (
        <button className="btn btn-small btn-ghost" onClick={() => setEditing(true)}>
          Edit
        </button>
      )}
      <button
        className="icon-btn"
        aria-label={`Delete memory "${m.content}"`}
        onClick={() => setConfirm(true)}
      >
        <Icon name="trash" size={16} />
      </button>
      {confirm && (
        <ConfirmDialog
          title="Delete memory?"
          confirmLabel="Delete"
          destructive
          onCancel={() => setConfirm(false)}
          onConfirm={() => {
            setConfirm(false);
            void host().memoryDelete(m.id).catch(onError);
          }}
        >
          <p>"{m.content}"</p>
        </ConfirmDialog>
      )}
    </li>
  );
}

export function MemoryTab() {
  const useMemory = useSettings((s) => s.snapshot?.settings.useMemory ?? true);
  const update = useSettings((s) => s.update);
  const [items, setItems] = useState<MemoryEntry[]>([]);
  const [content, setContent] = useState("");
  const [category, setCategory] = useState<MemoryCategory>("preference");
  const [confirmAll, setConfirmAll] = useState(false);
  const { status, fail, clear } = useStatus();

  const load = useCallback(() => {
    host()
      .memoryList()
      .then(setItems)
      .catch((e) => fail(toWispError(e)));
  }, [fail]);
  useReload("memory", load);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    clear();
    try {
      await host().memoryAdd(content, category);
      setContent("");
    } catch (err) {
      fail(toWispError(err));
    }
  };

  return (
    <div className="tab">
      <h2>Memory</h2>
      <p className="setting-desc">
        Short notes Wisp can use to personalise answers. They stay on this PC and are sent to Gemini
        with your requests only while memory is on. "Remember that…" saves a note straight away;
        anything picked up from your screen or clipboard asks first.
      </p>
      <section className="group">
        <Toggle
          id="memory-on"
          label="Use memory in requests"
          checked={useMemory}
          onChange={(v) => void update({ useMemory: v })}
        />
      </section>
      <form className="group" onSubmit={add}>
        <h3>Add a memory</h3>
        <div className="input-row">
          <label htmlFor="mem-new" className="sr-only">
            New memory
          </label>
          <input
            id="mem-new"
            type="text"
            value={content}
            maxLength={500}
            placeholder="e.g. I prefer metric units"
            onChange={(e) => setContent(e.target.value)}
          />
          <select
            aria-label="Category"
            className="select-inline"
            value={category}
            onChange={(e) => setCategory(e.target.value as MemoryCategory)}
          >
            {CATEGORIES.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
          <button className="btn btn-primary" disabled={!content.trim()}>
            Add
          </button>
        </div>
        <StatusLine status={status} />
      </form>
      <section className="group">
        <h3>Saved ({items.length})</h3>
        {items.length === 0 ? (
          <p className="muted">Nothing saved yet.</p>
        ) : (
          <ul className="item-list" aria-label="Memories">
            {items.map((m) => (
              <MemoryRow
                key={`${m.id}-${m.updatedAt}`}
                m={m}
                onError={(e) => fail(toWispError(e))}
              />
            ))}
          </ul>
        )}
        {items.length > 0 && (
          <div className="row-start">
            <button className="btn btn-danger-ghost" onClick={() => setConfirmAll(true)}>
              <Icon name="trash" size={16} /> Delete all memory
            </button>
          </div>
        )}
      </section>
      {confirmAll && (
        <ConfirmDialog
          title="Delete all memory?"
          confirmLabel="Delete all"
          destructive
          onCancel={() => setConfirmAll(false)}
          onConfirm={() => {
            setConfirmAll(false);
            void host()
              .dataDelete("memory")
              .catch((e) => fail(toWispError(e)));
          }}
        >
          <p>All {items.length} memories will be removed from this PC.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------

/** Stable fallback: a fresh `[]` per render would make the selector loop. */
const NO_APPS: AllowedApp[] = [];

export function ActionsTab() {
  const apps = useSettings((s) => s.snapshot?.settings.allowedApps ?? NO_APPS);
  const confirm = useSettings((s) => s.snapshot?.settings.confirmActions ?? "risky");
  const update = useSettings((s) => s.update);
  const [name, setName] = useState("");
  const [target, setTarget] = useState("");
  const [installed, setInstalled] = useState<InstalledApp[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const { status, ok, fail, clear } = useStatus();

  const loadInstalled = async () => {
    setLoading(true);
    try {
      setInstalled(await host().installedApps());
    } catch (e) {
      fail(toWispError(e));
    } finally {
      setLoading(false);
    }
  };
  const allowed = new Set(apps.map((a) => a.name));
  const q = query.trim().toLowerCase();
  const candidates = (installed ?? [])
    .filter((a) => !allowed.has(a.name) && (!q || a.name.includes(q)))
    .slice(0, 50);

  const save = async (next: AllowedApp[], msg: string): Promise<boolean> => {
    clear();
    const err = await update({ allowedApps: next });
    if (err) fail(err);
    else ok(msg);
    return !err;
  };

  return (
    <div className="tab">
      <h2>Actions</h2>
      <p className="setting-desc">
        Wisp can open and switch apps, open links, change Windows settings, copy text, set reminders
        and remember things when you ask. Wisp can't run commands, scripts or change files.
      </p>
      <section className="group">
        <h3>When to ask</h3>
        <Choice<"risky" | "always">
          name="confirm-actions"
          label="Ask before Wisp acts"
          value={confirm}
          options={[
            { value: "risky", label: "Only when it's risky" },
            { value: "always", label: "Always" },
          ]}
          onChange={(confirmActions) => void update({ confirmActions })}
        />
        <p className="setting-desc">
          {confirm === "risky"
            ? "What you ask for directly just happens. Wisp still asks before opening a link, before anything set for more than an hour from now, and whenever the request involves your screen, clipboard, a web search or window titles, since those can carry instructions you didn't write."
            : "Wisp asks you before every action."}
        </p>
      </section>
      <section className="group">
        <h3>Apps Wisp may open</h3>
        {apps.length === 0 ? (
          <p className="muted">No apps allowed.</p>
        ) : (
          <ul className="item-list" aria-label="Allowed apps">
            {apps.map((a) => (
              <li key={a.name} className="item">
                <div className="item-main">
                  <span className="item-title">{a.name}</span>
                  <span className="item-sub cell-mono">{a.target}</span>
                </div>
                <button
                  className="icon-btn"
                  aria-label={`Remove ${a.name}`}
                  onClick={() =>
                    void save(
                      apps.filter((x) => x.name !== a.name),
                      `Removed ${a.name}.`,
                    )
                  }
                >
                  <Icon name="trash" size={16} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className="group">
        <h3>Add from installed apps</h3>
        {installed === null ? (
          <div className="row-start">
            <button className="btn" onClick={() => void loadInstalled()} disabled={loading}>
              {loading ? "Looking…" : "Find installed apps"}
            </button>
          </div>
        ) : (
          <>
            <label htmlFor="app-search" className="sr-only">
              Search installed apps
            </label>
            <input
              id="app-search"
              type="search"
              value={query}
              placeholder="Search apps"
              onChange={(e) => setQuery(e.target.value)}
            />
            {candidates.length === 0 ? (
              <p className="muted">No matching apps.</p>
            ) : (
              <ul className="item-list app-picker" aria-label="Installed apps">
                {candidates.map((a) => (
                  <li key={a.target} className="item">
                    <div className="item-main">
                      <span className="item-title">{a.name}</span>
                    </div>
                    <button
                      className="btn btn-small"
                      onClick={() => void save([...apps, a], `Added ${a.name}.`)}
                      aria-label={`Allow ${a.name}`}
                    >
                      <Icon name="plus" size={14} /> Allow
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </section>
      <form
        className="group"
        onSubmit={(e) => {
          e.preventDefault();
          void save([...apps, { name, target }], `Added ${name.trim().toLowerCase()}.`).then(
            (saved) => {
              if (!saved) return;
              setName("");
              setTarget("");
            },
          );
        }}
      >
        <h3>Add an app manually</h3>
        <div className="form-grid">
          <div>
            <label htmlFor="app-name">Name Wisp should use</label>
            <input
              id="app-name"
              type="text"
              value={name}
              placeholder="e.g. spotify"
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div>
            <label htmlFor="app-target">Program or shortcut</label>
            <input
              id="app-target"
              type="text"
              value={target}
              placeholder="C:\Program Files\App\app.exe"
              onChange={(e) => setTarget(e.target.value)}
            />
          </div>
        </div>
        <p className="setting-desc">
          Use a program name like <code>notepad.exe</code>, or the full path to an .exe or
          Start-menu shortcut (.lnk). Shells and script hosts can't be added.
        </p>
        <div className="row-start">
          <button className="btn btn-primary" disabled={!name.trim() || !target.trim()}>
            Add app
          </button>
        </div>
        <StatusLine status={status} />
      </form>
    </div>
  );
}
