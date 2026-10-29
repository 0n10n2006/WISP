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
        Wisp listens with Windows' speech recognition, with no pop-up. The notch shows when it's
        listening, moves while it hears you, and says “Got it” when it's done. Hold the voice keys
        while you talk and let go when you've finished, or tap them once and Wisp stops when you
        pause (tap again to stop sooner). Dictation types into the app you're in; asking by voice
        fills Wisp's bar and asks. Wisp never records audio itself.
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
                  Hold or tap <kbd>{chordLabel(s.holdDictate)}</kbd> to dictate into any app.{" "}
                </>
              )}
              {s.holdAsk !== "off" && (
                <>
                  Hold or tap <kbd>{chordLabel(s.holdAsk)}</kbd> to ask by voice.{" "}
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
        <Toggle
          id="voice-sounds"
          label="Sounds when listening starts and stops"
          description="A soft rising note when Wisp starts listening and a falling one when it's done."
          checked={s.voiceSounds}
          disabled={!s.voiceEnabled}
          onChange={(voiceSounds) => void update({ voiceSounds })}
        />
      </section>
      <section className="group">
        <h3>Windows speech recognition</h3>
        <p className="setting-desc">
          Voice needs <strong>Online speech recognition</strong> turned on in Windows, an internet
          connection, and microphone access for desktop apps. The language follows Windows' speech
          language, and the microphone is Windows' default one. If Wisp says voice isn't available,
          it tells you which of these is missing.
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
