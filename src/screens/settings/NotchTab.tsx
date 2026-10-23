import { DASHBOARD_TABS } from "../../lib/commands";
import type {
  Accent,
  BallCorner,
  NotchPosition,
  NotchStyle,
  SliverStyle,
  Tone,
} from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, Toggle } from "./common";

/** Move `id` one place in `order` (only among the shown tabs). */
export function moveTab(order: string[], id: string, dir: -1 | 1): string[] {
  const i = order.indexOf(id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= order.length) return order;
  const next = [...order];
  [next[i], next[j]] = [next[j]!, next[i]!];
  return next;
}

/** Show or hide a tab; at least one always stays. */
export function toggleTab(order: string[], id: string, on: boolean): string[] {
  if (on) return order.includes(id) ? order : [...order, id];
  const next = order.filter((t) => t !== id);
  return next.length ? next : order;
}

function DashboardTabsEditor({
  value,
  onChange,
}: {
  value: string[];
  onChange(v: string[]): void;
}) {
  const label = (id: string) => DASHBOARD_TABS.find((t) => t.id === id)?.label ?? id;
  const hidden = DASHBOARD_TABS.filter((t) => !value.includes(t.id));
  return (
    <div className="tab-editor">
      <ul className="tab-order" aria-label="Dashboard tabs shown, in order">
        {value.map((id, i) => (
          <li key={id} className="tab-order-row">
            <input
              type="checkbox"
              id={`dash-show-${id}`}
              checked
              disabled={value.length === 1}
              onChange={() => onChange(toggleTab(value, id, false))}
            />
            <label htmlFor={`dash-show-${id}`}>{label(id)}</label>
            <button
              className="icon-btn icon-btn-small"
              aria-label={`Move ${label(id)} earlier`}
              disabled={i === 0}
              onClick={() => onChange(moveTab(value, id, -1))}
            >
              ↑
            </button>
            <button
              className="icon-btn icon-btn-small"
              aria-label={`Move ${label(id)} later`}
              disabled={i === value.length - 1}
              onClick={() => onChange(moveTab(value, id, 1))}
            >
              ↓
            </button>
          </li>
        ))}
        {hidden.map((t) => (
          <li key={t.id} className="tab-order-row tab-order-hidden">
            <input
              type="checkbox"
              id={`dash-show-${t.id}`}
              checked={false}
              onChange={() => onChange(toggleTab(value, t.id, true))}
            />
            <label htmlFor={`dash-show-${t.id}`}>{t.label}</label>
          </li>
        ))}
      </ul>
    </div>
  );
}

const ACCENTS: { value: Accent; label: string }[] = [
  { value: "cyan", label: "Cyan" },
  { value: "lavender", label: "Lavender" },
  { value: "ember", label: "Ember" },
  { value: "mint", label: "Mint" },
  { value: "custom", label: "Your colour" },
];

export function NotchTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  if (!s) return null;
  return (
    <div className="tab">
      <h2>Notch &amp; personality</h2>
      <section className="group">
        <h3>Where Wisp lives</h3>
        <p className="setting-desc">
          {s.notchStyle === "ball"
            ? "Wisp rests as a small ball in a corner of the screen. Hover it for the time, timer or what's playing; click or press the shortcut to open the command bar."
            : "Wisp rests as a thin glow at the top of the screen. Hover for the time, timer or what's playing; click or press the shortcut to open the command bar."}{" "}
          To hide it entirely, turn off “Show the notch” in General.
        </p>
        <Choice<NotchStyle>
          name="notch-style"
          label="Look"
          value={s.notchStyle}
          options={[
            { value: "notch", label: "Notch at the top" },
            { value: "ball", label: "Floating ball" },
          ]}
          onChange={(notchStyle) => void update({ notchStyle })}
        />
        {s.notchStyle === "ball" ? (
          <Choice<BallCorner>
            name="ball-corner"
            label="Corner"
            value={s.ballCorner}
            options={[
              { value: "top-left", label: "Top left" },
              { value: "top-right", label: "Top right" },
              { value: "bottom-left", label: "Bottom left" },
              { value: "bottom-right", label: "Bottom right" },
            ]}
            onChange={(ballCorner) => void update({ ballCorner })}
          />
        ) : (
          <Choice<NotchPosition>
            name="notch-position"
            label="Position"
            value={s.notchPosition}
            options={[
              { value: "left", label: "Left" },
              { value: "center", label: "Centre" },
              { value: "right", label: "Right" },
            ]}
            onChange={(notchPosition) => void update({ notchPosition })}
          />
        )}
        {s.notchStyle === "notch" && (
          <>
            <Choice<SliverStyle>
              name="sliver-style"
              label="When resting"
              value={s.sliverStyle}
              options={[
                { value: "glow", label: "Glow" },
                { value: "subtle", label: "Subtle" },
                { value: "hidden", label: "Invisible" },
              ]}
              onChange={(sliverStyle) => void update({ sliverStyle })}
            />
            <p className="setting-desc">
              {s.sliverStyle === "hidden"
                ? "Nothing shows until you hover the top edge or press the shortcut."
                : s.sliverStyle === "subtle"
                  ? "A faint line with no glow."
                  : "A soft accent line that brightens when Wisp is working."}
            </p>
          </>
        )}
        <Toggle
          id="close-on-blur"
          label="Close the bar when I click elsewhere"
          description="Esc or the shortcut always folds it."
          checked={s.closeOnBlur}
          onChange={(closeOnBlur) => void update({ closeOnBlur })}
        />
        <Toggle
          id="fold-on-leave"
          label="Fold away when the mouse leaves"
          description="The bar folds itself up a moment after you move away, unless you're typing, recording or waiting on an answer."
          checked={s.foldOnLeave}
          onChange={(foldOnLeave) => void update({ foldOnLeave })}
        />
        <Toggle
          id="tucked"
          label="Tuck Wisp away"
          description="Hide the notch completely. Shortcuts and the tray icon still work."
          checked={s.tucked}
          onChange={(tucked) => void update({ tucked })}
        />
        <div className="setting-row">
          <div className="setting-text">
            <label htmlFor="focus-minutes">Default focus timer</label>
          </div>
          <select
            id="focus-minutes"
            className="select-inline"
            value={s.focusMinutes}
            onChange={(e) => void update({ focusMinutes: Number(e.target.value) })}
          >
            {[10, 15, 20, 25, 30, 45, 50, 60, 90].map((m) => (
              <option key={m} value={m}>
                {m} minutes
              </option>
            ))}
          </select>
        </div>
        <fieldset className="setting-row choice">
          <legend>Accent colour</legend>
          <div className="swatches" role="radiogroup" aria-label="Accent colour">
            {ACCENTS.map((a) => (
              <label
                key={a.value}
                className={`swatch swatch-${a.value} ${s.accent === a.value ? "selected" : ""}`}
              >
                <input
                  type="radio"
                  name="accent"
                  value={a.value}
                  checked={s.accent === a.value}
                  onChange={() => void update({ accent: a.value })}
                />
                <span className="swatch-dot" aria-hidden />
                {a.label}
              </label>
            ))}
          </div>
        </fieldset>
        {s.accent === "custom" && (
          <div className="setting-row">
            <div className="setting-text">
              <label htmlFor="accent-color">Your colour</label>
            </div>
            <input
              id="accent-color"
              type="color"
              className="color-input"
              value={s.accentColor}
              onChange={(e) => void update({ accentColor: e.target.value })}
            />
          </div>
        )}
      </section>
      <section className="group">
        <h3>Dashboard tabs</h3>
        <p className="setting-desc">Choose which tabs the bar shows and in what order.</p>
        <DashboardTabsEditor
          value={s.dashboardTabs}
          onChange={(dashboardTabs) => void update({ dashboardTabs })}
        />
      </section>
      <section className="group">
        <h3>Speed</h3>
        <Toggle
          id="fast-mode"
          label="Fast answers"
          description="Skips the model's thinking step for near-instant replies. Use “Think” in the bar (or Shift+Enter) when a question deserves more care."
          checked={s.fastMode}
          onChange={(fastMode) => void update({ fastMode })}
        />
      </section>
      <section className="group">
        <h3>Personality</h3>
        <Choice<Tone>
          name="tone"
          label="Tone"
          value={s.persona.tone}
          options={[
            { value: "concise", label: "Concise" },
            { value: "friendly", label: "Friendly" },
            { value: "professional", label: "Professional" },
          ]}
          onChange={(tone) => void update({ persona: { tone } })}
        />
        <label htmlFor="persona">Custom instructions</label>
        <p className="setting-desc" id="persona-desc">
          Anything Wisp should always keep in mind, e.g. “Use British spelling” or “I'm a nurse,
          keep medical terms”. Sent with each question. Up to 1,000 characters.
        </p>
        <textarea
          id="persona"
          className="textarea"
          rows={4}
          maxLength={1000}
          aria-describedby="persona-desc"
          defaultValue={s.persona.instructions}
          onBlur={(e) => {
            if (e.target.value !== s.persona.instructions)
              void update({ persona: { instructions: e.target.value } });
          }}
        />
      </section>
    </div>
  );
}
