import { useMemo, useState, type ReactNode } from "react";
import { Icon } from "../../components/Icon";
import { host } from "../../services/host";
import type { WispError } from "../../services/errors";

export const LINKS = {
  getKey: "https://aistudio.google.com/apikey",
  terms: "https://ai.google.dev/gemini-api/terms",
  billing: "https://ai.google.dev/gemini-api/docs/billing",
  rateLimits: "https://ai.google.dev/gemini-api/docs/rate-limits",
  privacy: "https://policies.google.com/privacy",
};

export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      className="ext-link"
      onClick={(e) => {
        e.preventDefault();
        void host().openExternal(href);
      }}
    >
      {children}
      <Icon name="external" size={14} />
    </a>
  );
}

export function Toggle({
  id,
  label,
  description,
  checked,
  onChange,
  disabled,
}: {
  id: string;
  label: string;
  description?: ReactNode;
  checked: boolean;
  onChange(v: boolean): void;
  disabled?: boolean;
}) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <label htmlFor={id}>{label}</label>
        {description && (
          <p className="setting-desc" id={`${id}-desc`}>
            {description}
          </p>
        )}
      </div>
      <input
        id={id}
        type="checkbox"
        role="switch"
        className="switch"
        checked={checked}
        disabled={disabled}
        aria-describedby={description ? `${id}-desc` : undefined}
        onChange={(e) => onChange(e.target.checked)}
      />
    </div>
  );
}

export function Choice<T extends string>({
  name,
  label,
  value,
  options,
  onChange,
}: {
  name: string;
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange(v: T): void;
}) {
  return (
    <fieldset className="setting-row choice">
      <legend>{label}</legend>
      <div className="segmented" role="radiogroup" aria-label={label}>
        {options.map((o) => (
          <label key={o.value} className={value === o.value ? "selected" : ""}>
            <input
              type="radio"
              name={name}
              value={o.value}
              checked={value === o.value}
              onChange={() => onChange(o.value)}
            />
            {o.label}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** Inline result line for a settings action. */
export function useStatus() {
  const [status, setStatus] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const actions = useMemo(
    () => ({
      ok: (text: string) => setStatus({ tone: "ok", text }),
      fail: (e: WispError | string) =>
        setStatus({ tone: "error", text: typeof e === "string" ? e : e.message }),
      clear: () => setStatus(null),
    }),
    [],
  );
  return { status, ...actions };
}

export function StatusLine({ status }: { status: { tone: "ok" | "error"; text: string } | null }) {
  if (!status) return null;
  return (
    <p
      className={status.tone === "ok" ? "field-ok" : "field-error"}
      role={status.tone === "error" ? "alert" : "status"}
    >
      <Icon name={status.tone === "ok" ? "check" : "warning"} size={14} /> {status.text}
    </p>
  );
}
