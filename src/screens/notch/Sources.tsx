import { Icon } from "../../components/Icon";
import { host } from "../../services/host";
import type { Source } from "../../services/types";
import { toast } from "../../state/toast";

function hostName(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Source chips for a web-grounded answer. Links open in the default browser. */
export function Sources({ sources }: { sources: Source[] }) {
  return (
    <div className="sources" aria-label="Sources">
      <span className="sources-label">
        <Icon name="globe" size={13} /> Sources
      </span>
      {sources.slice(0, 8).map((s, i) => (
        <button
          key={`${s.url}-${i}`}
          className="source-chip"
          title={s.url}
          onClick={() =>
            void host()
              .openExternal(s.url)
              .catch(() => toast("Couldn't open that link", "warning"))
          }
        >
          <span className="source-n">{i + 1}</span>
          {s.title && s.title !== hostName(s.url) ? s.title : hostName(s.url)}
        </button>
      ))}
    </div>
  );
}
