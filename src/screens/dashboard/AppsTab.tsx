import { useMemo, useState } from "react";
import { Icon } from "../../components/Icon";
import { rankApps } from "../../lib/commands";
import { toWispError } from "../../services/errors";
import { host } from "../../services/host";
import type { LaunchItem } from "../../services/types";
import { useSettings } from "../../state/settings";
import { toast } from "../../state/toast";
import { invalidateCatalogue, useCatalogue } from "../../lib/catalogue";

const NO_STARS: string[] = [];

function hue(name: string): number {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

function Tile({ item, starred }: { item: LaunchItem; starred: boolean }) {
  const toggleStar = () => {
    const s = useSettings.getState().snapshot?.settings.starredApps ?? [];
    const next = starred ? s.filter((x) => x !== item.id) : [...s, item.id].slice(-50);
    void useSettings.getState().update({ starredApps: next });
  };
  return (
    <li className="app-tile">
      <button
        className="app-launch"
        onClick={() => {
          invalidateCatalogue();
          void host()
            .launchItem(item)
            .then(() => host().hideIsland())
            .catch((e) => toast(toWispError(e).message, "warning"));
        }}
        title={`Open ${item.name}`}
      >
        <span
          className="app-avatar"
          style={{ "--h": hue(item.name) } as React.CSSProperties}
          aria-hidden
        >
          {item.kind === "game" ? (
            <Icon name="play" size={14} />
          ) : item.kind === "system" ? (
            <Icon name="settings" size={14} />
          ) : (
            item.name.slice(0, 1).toUpperCase()
          )}
        </span>
        <span className="app-name">{item.name}</span>
      </button>
      <button
        className={`app-star ${starred ? "on" : ""}`}
        aria-label={starred ? `Unstar ${item.name}` : `Star ${item.name}`}
        aria-pressed={starred}
        onClick={toggleStar}
      >
        <Icon name="star" size={13} />
      </button>
    </li>
  );
}

function Section({ title, items, stars }: { title: string; items: LaunchItem[]; stars: string[] }) {
  if (!items.length) return null;
  return (
    <section className="app-section">
      <h3>{title}</h3>
      <ul className="app-grid">
        {items.map((i) => (
          <Tile key={i.id} item={i} starred={stars.includes(i.id)} />
        ))}
      </ul>
    </section>
  );
}

export default function AppsTab() {
  const cat = useCatalogue();
  const stars = useSettings((s) => s.snapshot?.settings.starredApps ?? NO_STARS);
  const [q, setQ] = useState("");
  const groups = useMemo(() => {
    const items = cat?.items ?? [];
    const byId = new Map(items.map((i) => [i.id, i]));
    if (q.trim())
      return { results: rankApps(q, items, stars, cat?.recent ?? [], 24).map((r) => r.item) };
    return {
      starred: stars.map((id) => byId.get(id)).filter((x): x is LaunchItem => !!x),
      recent: (cat?.recent ?? [])
        .map((r) => byId.get(r.id))
        .filter((x): x is LaunchItem => !!x && !stars.includes(x.id))
        .slice(0, 8),
      apps: items.filter((i) => i.kind === "app"),
      games: items.filter((i) => i.kind === "game"),
      system: items.filter((i) => i.kind === "system"),
    };
  }, [cat, stars, q]);
  if (!cat) return <div className="dash-loading" aria-busy="true" />;
  return (
    <div className="apps">
      <div className="row">
        <input
          className="input"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={`Search ${cat.items.length} apps, games and settings`}
          aria-label="Search apps"
        />
        <button
          className="icon-btn icon-btn-small"
          aria-label="Rescan installed apps and games"
          title="Rescan installed apps and games"
          onClick={() => invalidateCatalogue(true)}
        >
          <Icon name="restart" size={15} />
        </button>
      </div>
      {"results" in groups ? (
        groups.results && groups.results.length ? (
          <Section title="Results" items={groups.results} stars={stars} />
        ) : (
          <p className="muted pad">No matches.</p>
        )
      ) : (
        <>
          <Section title="Starred" items={groups.starred ?? []} stars={stars} />
          <Section title="Recent" items={groups.recent ?? []} stars={stars} />
          <Section title="Games" items={groups.games ?? []} stars={stars} />
          <Section title="Apps" items={groups.apps ?? []} stars={stars} />
          <Section title="Windows" items={groups.system ?? []} stars={stars} />
        </>
      )}
    </div>
  );
}
