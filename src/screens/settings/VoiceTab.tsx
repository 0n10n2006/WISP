import { useEffect, useState } from "react";
import { chordLabel, type DictationStyle } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, Toggle } from "./common";

const LANGUAGES: { value: string; label: string }[] = [
  { value: "", label: "Detect automatically" },
  { value: "en-US", label: "English (US)" },
  { value: "en-GB", label: "English (UK)" },
  { value: "en-IN", label: "English (India)" },
  { value: "es", label: "Español" },
  { value: "fr", label: "Français" },
  { value: "de", label: "Deutsch" },
  { value: "it", label: "Italiano" },
  { value: "pt-BR", label: "Português (Brasil)" },
  { value: "nl", label: "Nederlands" },
  { value: "pl", label: "Polski" },
  { value: "tr", label: "Türkçe" },
  { value: "ru", label: "Русский" },
  { value: "ar", label: "العربية" },
  { value: "hi", label: "हिन्दी" },
  { value: "ur", label: "اردو" },
  { value: "bn", label: "বাংলা" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "zh-CN", label: "中文 (简体)" },
];

/** Split a textarea of names (one per line or comma-separated). */
export function parseVocabulary(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of text.split(/[\n,]/)) {
    const w = raw.trim().slice(0, 60);
    if (w && !seen.has(w.toLowerCase())) {
      seen.add(w.toLowerCase());
      out.push(w);
    }
  }
  return out.slice(0, 100);
}

function useMicrophones(): { id: string; label: string }[] {
  const [mics, setMics] = useState<{ id: string; label: string }[]>([]);
  useEffect(() => {
    const md = typeof navigator !== "undefined" ? navigator.mediaDevices : undefined;
    if (!md?.enumerateDevices) return;
    const load = () =>
      md
        .enumerateDevices()
        .then((list) =>
          setMics(
            list
              .filter((d) => d.kind === "audioinput" && d.deviceId && d.deviceId !== "default")
              .map((d, i) => ({ id: d.deviceId, label: d.label || `Microphone ${i + 1}` })),
          ),
        )
        .catch(() => setMics([]));
    void load();
    md.addEventListener?.("devicechange", load);
    return () => md.removeEventListener?.("devicechange", load);
  }, []);
  return mics;
}

export function VoiceTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  const mics = useMicrophones();
  if (!s) return null;
  const pause = (s.silenceMs / 1000).toFixed(1);

  return (
    <div className="tab">
      <h2>Voice &amp; dictation</h2>
      <p className="setting-desc">
        Wisp listens only while you hold a key or press a button, never in the background. Speech is
        transcribed by Gemini. Wisp answers in text and never talks back.
      </p>
      <section className="group">
        <Toggle
          id="voice-enabled"
          label="Allow voice and dictation"
          checked={s.voiceEnabled}
          onChange={(voiceEnabled) => void update({ voiceEnabled })}
        />
        <Toggle
          id="hold-to-talk"
          label="Hold-to-talk keys"
          description={
            <>
              {s.holdDictate !== "off" && (
                <>
                  Hold <kbd>{chordLabel(s.holdDictate)}</kbd> to dictate into any app and release to
                  type it in.{" "}
                </>
              )}
              {s.holdAsk !== "off" && (
                <>
                  Hold <kbd>{chordLabel(s.holdAsk)}</kbd> to ask by voice.{" "}
                </>
              )}
              Pressing any other key while holding them keeps your normal Windows shortcuts working.
              Change the keys in Shortcuts.
            </>
          }
          checked={s.holdToTalk}
          disabled={!s.voiceEnabled}
          onChange={(holdToTalk) => void update({ holdToTalk })}
        />
      </section>
      <section className="group">
        <h3>Listening</h3>
        <Toggle
          id="auto-stop"
          label="Stop when I'm done talking"
          description="Wisp notices when you pause and stops listening by itself, so you don't have to press anything. Holding the keys always listens until you let go."
          checked={s.autoStopVoice}
          disabled={!s.voiceEnabled}
          onChange={(autoStopVoice) => void update({ autoStopVoice })}
        />
        <div className="setting-row">
          <div className="setting-text">
            <label htmlFor="silence-ms">Pause that means “done”</label>
            <p className="setting-desc" id="silence-desc">
              Shorter feels snappier; longer gives you time to think mid-sentence.
            </p>
          </div>
          <div className="range-row">
            <input
              id="silence-ms"
              type="range"
              min={500}
              max={4000}
              step={100}
              value={s.silenceMs}
              aria-describedby="silence-desc"
              aria-valuetext={`${pause} seconds`}
              disabled={!s.voiceEnabled || !s.autoStopVoice}
              onChange={(e) => void update({ silenceMs: Number(e.target.value) })}
            />
            <output htmlFor="silence-ms">{pause} s</output>
          </div>
        </div>
        <div className="setting-row">
          <div className="setting-text">
            <label htmlFor="mic-device">Microphone</label>
          </div>
          <select
            id="mic-device"
            className="select-inline"
            value={s.micDevice}
            onChange={(e) => void update({ micDevice: e.target.value })}
          >
            <option value="">Windows default</option>
            {s.micDevice && !mics.some((m) => m.id === s.micDevice) && (
              <option value={s.micDevice}>Saved microphone (not connected)</option>
            )}
            {mics.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </div>
        <div className="setting-row">
          <div className="setting-text">
            <label htmlFor="voice-language">Language you speak</label>
            <p className="setting-desc" id="lang-desc">
              Telling Wisp your language makes recognition more accurate, especially for accents.
            </p>
          </div>
          <select
            id="voice-language"
            className="select-inline"
            aria-describedby="lang-desc"
            value={s.voiceLanguage}
            onChange={(e) => void update({ voiceLanguage: e.target.value })}
          >
            {!LANGUAGES.some((l) => l.value === s.voiceLanguage) && (
              <option value={s.voiceLanguage}>{s.voiceLanguage}</option>
            )}
            {LANGUAGES.map((l) => (
              <option key={l.value} value={l.value}>
                {l.label}
              </option>
            ))}
          </select>
        </div>
        <label htmlFor="vocabulary">Words to get right</label>
        <p className="setting-desc" id="vocab-desc">
          Names, products and jargon Wisp should spell exactly, one per line, e.g. your name, your
          company, “Kubernetes”. Installed app names are included automatically.
        </p>
        <textarea
          id="vocabulary"
          className="textarea"
          rows={3}
          aria-describedby="vocab-desc"
          defaultValue={s.vocabulary.join("\n")}
          onBlur={(e) => {
            const vocabulary = parseVocabulary(e.target.value);
            if (vocabulary.join("\n") !== s.vocabulary.join("\n")) void update({ vocabulary });
          }}
        />
      </section>
      <section className="group">
        <h3>Dictation</h3>
        <Choice<DictationStyle>
          name="dictation-style"
          label="Clean-up"
          value={s.dictationStyle}
          options={[
            { value: "verbatim", label: "Verbatim" },
            { value: "clean", label: "Clean" },
            { value: "formal", label: "Polished" },
          ]}
          onChange={(dictationStyle) => void update({ dictationStyle })}
        />
        <p className="setting-desc">
          {s.dictationStyle === "verbatim"
            ? "Exactly what you said, with punctuation."
            : s.dictationStyle === "clean"
              ? "Removes filler words (um, uh, like), false starts and repetitions, and fixes grammar — in your own words."
              : "Turns what you said into clear, professional writing."}
        </p>
        <Toggle
          id="auto-insert"
          label="Type dictation straight into the app"
          description="When on, the dictation button and shortcut insert text without a review step. Hold-to-talk always inserts on release."
          checked={s.dictationAutoInsert}
          onChange={(dictationAutoInsert) => void update({ dictationAutoInsert })}
        />
      </section>
    </div>
  );
}
