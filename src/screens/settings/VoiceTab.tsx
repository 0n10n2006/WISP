import type { DictationStyle } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Choice, Toggle } from "./common";

export function VoiceTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  if (!s) return null;

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
              Hold <kbd>Ctrl</kbd>+<kbd>Win</kbd> to dictate into any app and release to type it in.
              Hold <kbd>Win</kbd>+<kbd>Alt</kbd> to ask by voice. Pressing any other key while
              holding them keeps your normal Windows shortcuts working.
            </>
          }
          checked={s.holdToTalk}
          disabled={!s.voiceEnabled}
          onChange={(holdToTalk) => void update({ holdToTalk })}
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
