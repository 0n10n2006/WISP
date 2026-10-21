import type { Accent, Tone } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, Toggle } from "./common";

const ACCENTS: { value: Accent; label: string }[] = [
  { value: "cyan", label: "Cyan" },
  { value: "lavender", label: "Lavender" },
  { value: "ember", label: "Ember" },
  { value: "mint", label: "Mint" },
];

export function NotchTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  if (!s) return null;
  return (
    <div className="tab">
      <h2>Notch &amp; personality</h2>
      <section className="group">
        <h3>The notch</h3>
        <p className="setting-desc">
          Wisp rests as a thin glow at the top of the screen. Hover for the time, timer or what's
          playing; click or press the shortcut to open the command bar.
        </p>
        <Toggle
          id="auto-hide"
          label="Hide while a fullscreen app is in front"
          description="Games, videos and presentations stay unobstructed."
          checked={s.autoHideFullscreen}
          onChange={(autoHideFullscreen) => void update({ autoHideFullscreen })}
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
