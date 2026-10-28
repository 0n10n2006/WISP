import { host } from "../../services/host";
import { chordLabel } from "../../services/types";
import { useSettings } from "../../state/settings";
import { Toggle } from "./common";

/** Windows' speech privacy page (online speech recognition). */
const SPEECH_SETTINGS = "ms-settings:privacy-speech";

export function VoiceTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  if (!s) return null;

  return (
    <div className="tab">
      <h2>Voice &amp; dictation</h2>
      <p className="setting-desc">
        Wisp listens with Windows' speech recognition: no pop-up, the words show in the notch as you
        speak, and it stops by itself when you pause. Dictation types into the app you're in; asking
        by voice fills Wisp's bar and asks when you stop. Tap the keys again to stop early. Wisp
        never records audio itself.
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
        <h3>Windows speech recognition</h3>
        <p className="setting-desc">
          Voice needs <strong>Online speech recognition</strong> turned on in Windows. The language
          follows Windows' speech language, and the microphone is Windows' default one.
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
