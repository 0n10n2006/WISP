import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import { DEFAULT_WEB_SHORTCUTS, TAG_LABELS, TAG_PREFIX, type SearchTag } from "../../lib/commands";
import type { FilesStatus, WebShortcut } from "../../services/types";
import { useSettings } from "../../state/settings";
import { StatusLine, Toggle, useStatus } from "./common";

/** "2 min ago", "just now", "3 h ago". */
export function ago(ms: number, now = Date.now()): string {
  const mins = Math.floor((now - ms) / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const h = Math.round(mins / 60);
  if (h < 24) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}

export function statusText(st: FilesStatus): string {
  if (!st.enabled) return "File search is off.";
  if (st.indexing)
    return `Indexing… ${st.scanned.toLocaleString()} found so far. Search works while it runs.`;
  const files = `${st.count.toLocaleString()} ${st.count === 1 ? "item" : "items"}`;
  return st.updatedAt ? `${files} · updated ${ago(st.updatedAt)}` : files;
}

export function SearchTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  const [files, setFiles] = useState<FilesStatus | null>(null);
  const { status, ok, fail, clear } = useStatus();

  useEffect(() => {
    let live = true;
    void host()
      .filesStatus()
      .then((st) => live && setFiles(st))
      .catch(() => {});
    const off = host().onFilesIndex((st) => live && setFiles(st));
    return () => {
      live = false;
      off();
    };
  }, [s?.indexFiles, s?.indexFolders]);

  if (!s) return null;
  const run = async (fn: () => Promise<FilesStatus>, done?: string) => {
    clear();
    try {
      setFiles(await fn());
      if (done) ok(done);
    } catch (e) {
      fail(toWispError(e));
    }
  };

  return (
    <div className="tab">
      <h2>Search</h2>
      <p className="setting-desc">
        Wisp keeps a list of the names and places of the files in your folders, so typing part of a
        name in the bar (or saying “open my resume”) finds it at once. File names stay on this PC;
        Wisp never reads what's inside your files. Programs and scripts are never listed or opened.
      </p>
      <section className="group">
        <Toggle
          id="index-files"
          label="Index my files"
          description={files ? statusText(files) : undefined}
          checked={s.indexFiles}
          onChange={(indexFiles) => void update({ indexFiles })}
        />
        {s.indexFiles && (
          <div className="row-start">
            <button
              className="btn"
              disabled={files?.indexing}
              onClick={() => void run(() => host().filesRebuild(), "Indexing again.")}
            >
              <Icon name="retry" size={14} /> Index again now
            </button>
          </div>
        )}
      </section>
      {s.indexFiles && (
        <section className="group">
          <h3>Folders</h3>
          <p className="setting-desc">
            Desktop, Documents, Downloads, Pictures, Music, Videos and OneDrive are included. Add
            other folders, such as a projects drive.
          </p>
          <ul className="item-list" aria-label="Indexed folders">
            {(files?.folders ?? []).map((f) => (
              <li key={f.path} className="item">
                <Icon name="folder" size={16} />
                <div className="item-main">
                  <span className="item-title">{f.label}</span>
                  <span className="item-sub cell-mono">{f.path}</span>
                </div>
                {f.user && (
                  <button
                    className="icon-btn"
                    aria-label={`Stop indexing ${f.label}`}
                    onClick={() =>
                      void run(
                        () => host().filesFolderRemove(f.path),
                        `${f.label} is no longer indexed.`,
                      )
                    }
                  >
                    <Icon name="trash" size={16} />
                  </button>
                )}
              </li>
            ))}
          </ul>
          <div className="row-start">
            <button className="btn" onClick={() => void run(() => host().filesFolderAdd())}>
              <Icon name="plus" size={14} /> Add a folder
            </button>
          </div>
          <StatusLine status={status} />
        </section>
      )}
      <BrowserSection />
      <ShortcutsSection />
      <section className="group">
        <h3>Search tags</h3>
        <p className="setting-desc">
          Start with a tag to search just one thing, or type the word and press Tab. Backspace
          removes it.
        </p>
        <ul className="plain-list tag-list">
          {(Object.keys(TAG_LABELS) as SearchTag[]).map((t) => (
            <li key={t}>
              <kbd>{TAG_PREFIX[t]}</kbd> {TAG_LABELS[t]}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function BrowserSection() {
  const on = useSettings((st) => st.snapshot?.settings.browserSearch ?? true);
  const update = useSettings((st) => st.update);
  const [found, setFound] = useState<string[] | null>(null);
  useEffect(() => {
    let live = true;
    void host()
      .browsersInstalled()
      .then((b) => live && setFound(b))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);
  return (
    <section className="group">
      <Toggle
        id="browser-search"
        label="Search browser tabs and history"
        description={
          <>
            Open tabs, bookmarks and pages you've visited show in the bar. Wisp reads them from your
            browsers when you search, keeps nothing and never sends them anywhere.
            {found && (
              <> {found.length ? `Found: ${found.join(", ")}.` : "No supported browser found."}</>
            )}
          </>
        }
        checked={on}
        onChange={(browserSearch) => void update({ browserSearch })}
      />
    </section>
  );
}

function ShortcutsSection() {
  const list = useSettings((st) => st.snapshot?.settings.webShortcuts ?? DEFAULT_WEB_SHORTCUTS);
  const update = useSettings((st) => st.update);
  const { status, ok, fail, clear } = useStatus();
  const [draft, setDraft] = useState<WebShortcut>({ prefix: "", name: "", url: "" });
  const save = async (webShortcuts: WebShortcut[], done: string) => {
    clear();
    const err = await update({ webShortcuts });
    if (err) fail(err);
    else ok(done);
    return !err;
  };
  return (
    <section className="group">
      <h3>Web search shortcuts</h3>
      <p className="setting-desc">
        Type a shortcut and your search, like <kbd>g</kbd> cats or <kbd>yt</kbd> lofi, and Enter
        opens it in your browser.
      </p>
      <ul className="item-list" aria-label="Web search shortcuts">
        {list.map((w) => (
          <li key={w.prefix} className="item">
            <kbd>{w.prefix}</kbd>
            <div className="item-main">
              <span className="item-title">{w.name}</span>
              <span className="item-sub cell-mono">{w.url}</span>
            </div>
            <button
              className="icon-btn"
              aria-label={`Remove ${w.name}`}
              onClick={() =>
                void save(
                  list.filter((x) => x.prefix !== w.prefix),
                  `Removed ${w.name}.`,
                )
              }
            >
              <Icon name="trash" size={16} />
            </button>
          </li>
        ))}
      </ul>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save([...list, draft], `Added ${draft.name}.`).then(
            (saved) => saved && setDraft({ prefix: "", name: "", url: "" }),
          );
        }}
      >
        <div className="form-grid">
          <div>
            <label htmlFor="ws-prefix">Shortcut</label>
            <input
              id="ws-prefix"
              type="text"
              value={draft.prefix}
              placeholder="e.g. npm"
              maxLength={10}
              onChange={(e) => setDraft({ ...draft, prefix: e.target.value })}
            />
          </div>
          <div>
            <label htmlFor="ws-name">Name</label>
            <input
              id="ws-name"
              type="text"
              value={draft.name}
              placeholder="e.g. npm"
              maxLength={40}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
          </div>
        </div>
        <label htmlFor="ws-url">Address, with %s for the search</label>
        <input
          id="ws-url"
          type="url"
          value={draft.url}
          placeholder="https://www.npmjs.com/search?q=%s"
          maxLength={500}
          onChange={(e) => setDraft({ ...draft, url: e.target.value })}
        />
        <div className="row-start">
          <button
            className="btn btn-primary"
            disabled={!draft.prefix.trim() || !draft.name.trim() || !draft.url.trim()}
          >
            <Icon name="plus" size={14} /> Add shortcut
          </button>
          <button
            type="button"
            className="btn"
            onClick={() => void save(DEFAULT_WEB_SHORTCUTS, "Shortcuts reset.")}
          >
            Reset to defaults
          </button>
        </div>
      </form>
      <StatusLine status={status} />
    </section>
  );
}
