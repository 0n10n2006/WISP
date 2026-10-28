import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { FilesStatus } from "../../services/types";
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
    </div>
  );
}
