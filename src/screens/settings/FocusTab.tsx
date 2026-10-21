import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { host } from "../../services/host";
import type { Cover, OpenWindow } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Toggle } from "./common";

/** "C:\Games\Foo.EXE" or "Foo.exe" → "foo" (matches the Rust side). */
export function processName(text: string): string {
  const base = text.trim().split(/[\\/]/).pop() ?? "";
  return base
    .replace(/\.exe$/i, "")
    .toLowerCase()
    .slice(0, 100);
}

const COVER_TEXT: Record<Cover, string> = {
  show: "Showing: nothing in front needs Wisp out of the way.",
  fullscreen: "Hidden: a fullscreen app is in front.",
  game: "Hidden: you're in a game. Hold-to-talk and shortcuts are paused.",
  presentation: "Hidden: Windows is in presentation mode.",
  excluded: "Hidden: an app on your “never over” list is in front.",
};

export function FocusTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  const [cover, setCover] = useState<Cover | null>(null);
  const [windows, setWindows] = useState<OpenWindow[]>([]);
  const [name, setName] = useState("");

  useEffect(() => {
    let live = true;
    const poll = () =>
      host()
        .screenStatus()
        .then((c) => live && setCover(c))
        .catch(() => {});
    void poll();
    const t = setInterval(poll, 2000);
    host()
      .openWindows()
      .then((w) => live && setWindows(w))
      .catch(() => {});
    return () => {
      live = false;
      clearInterval(t);
    };
  }, []);

  if (!s) return null;
  const never = s.neverOverApps;
  const add = (app: string) => {
    const n = processName(app);
    if (!n || never.includes(n)) return;
    void update({ neverOverApps: [...never, n].sort() });
  };
  const remove = (app: string) => void update({ neverOverApps: never.filter((a) => a !== app) });
  const suggestions = [...new Set(windows.map((w) => w.app))].filter(
    (a) => a && a !== "app" && !never.includes(a),
  );

  return (
    <div className="tab">
      <h2>Games &amp; focus</h2>
      <p className="setting-desc">
        Wisp watches which window is in front (never its contents) so it can get out of the way.
        {cover && (
          <>
            {" "}
            <strong role="status">{COVER_TEXT[cover]}</strong>
          </>
        )}
      </p>
      <section className="group">
        <h3>Stay out of the way</h3>
        <Toggle
          id="hide-in-games"
          label="Never show over games"
          description="Detected from Windows' own game mode, fullscreen DirectX and the usual game folders (Steam, Epic, Xbox, Riot, GOG, Ubisoft, EA, Battle.net). Works on the monitor the game is on."
          checked={s.hideInGames}
          onChange={(hideInGames) => void update({ hideInGames })}
        />
        <Toggle
          id="shortcuts-in-games"
          label="Keep shortcuts working in games"
          description="Off by default, so Wisp's keys and hold-to-talk never steal a key press from a game."
          checked={s.shortcutsInGames}
          disabled={!s.hideInGames}
          onChange={(shortcutsInGames) => void update({ shortcutsInGames })}
        />
        <Toggle
          id="auto-hide"
          label="Hide while a fullscreen app is in front"
          description="Videos, presentations and anything else that fills the screen stay unobstructed."
          checked={s.autoHideFullscreen}
          onChange={(autoHideFullscreen) => void update({ autoHideFullscreen })}
        />
      </section>
      <section className="group">
        <h3>Never show over these apps</h3>
        <p className="setting-desc">
          Wisp hides whenever one of these is in front on its monitor, fullscreen or not.
        </p>
        <form
          className="input-row"
          onSubmit={(e) => {
            e.preventDefault();
            add(name);
            setName("");
          }}
        >
          <input
            type="text"
            aria-label="App to add (process name, e.g. obs64)"
            placeholder="Process name, e.g. obs64 or photoshop"
            value={name}
            maxLength={260}
            spellCheck={false}
            onChange={(e) => setName(e.target.value)}
          />
          <button className="btn" disabled={!processName(name) || never.length >= 50}>
            Add
          </button>
        </form>
        {never.length > 0 ? (
          <ul className="app-list" aria-label="Apps Wisp never shows over">
            {never.map((a) => (
              <li key={a} className="app-chip">
                {a}
                <button
                  className="icon-btn icon-btn-small"
                  aria-label={`Remove ${a}`}
                  onClick={() => remove(a)}
                >
                  <Icon name="close" size={12} />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="setting-desc">No apps yet.</p>
        )}
        {suggestions.length > 0 && never.length < 50 && (
          <>
            <p className="setting-desc">Open right now:</p>
            <ul className="app-list" aria-label="Open apps you can add">
              {suggestions.map((a) => (
                <li key={a}>
                  <button className="btn btn-small" onClick={() => add(a)}>
                    + {a}
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
