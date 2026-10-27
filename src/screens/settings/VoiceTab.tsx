import { host } from "../../services/host";
import { chordLabel } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Toggle } from "./common";

/** Windows' speech settings (online speech recognition, voice typing). */
const SPEECH_SETTINGS = "ms-settings:speech";

export function VoiceTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  if (!s) return null;

  return (
    <div className="tab">
      <h2>Voice &amp; dictation</h2>
      <p className="setting-desc">
        Voice uses Windows voice typing: words appear as you speak, punctuated, and it stops by
        itself when you pause. Dictation types into the app you're in; asking by voice types into
        Wisp's bar and asks when you stop. Wisp never records audio itself.
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
          label="Voice keys"
          description={
            <>
              {s.holdDictate !== "off" && (
                <>
                  Tap <kbd>{chordLabel(s.holdDictate)}</kbd> to dictate into any app.{" "}
                </>
              )}
              {s.holdAsk !== "off" && (
                <>
                  Tap <kbd>{chordLabel(s.holdAsk)}</kbd> to ask by voice.{" "}
                </>
              )}
              Pressing any other key with them keeps your normal Windows shortcuts working. Change
              the keys in Shortcuts.
            </>
          }
          checked={s.holdToTalk}
          disabled={!s.voiceEnabled}
          onChange={(holdToTalk) => void update({ holdToTalk })}
        />
      </section>
      <section className="group">
        <h3>Windows voice typing</h3>
        <p className="setting-desc">
          If Windows says voice typing isn't available, turn on{" "}
          <strong>Online speech recognition</strong> in Windows' speech settings. Your language,
          microphone and automatic punctuation are set in Windows too (press Win+H, then the gear).
        </p>
        <div className="row-start">
          <button
            className="btn"
            onClick={() =>
              void host()
                .launchItem({
                  id: `uri:${SPEECH_SETTINGS}`,
                  name: "Speech settings",
                  kind: "system",
                  target: SPEECH_SETTINGS,
                })
                .catch(() => {})
            }
          >
            Open Windows speech settings
          </button>
        </div>
      </section>
    </div>
  );
}
