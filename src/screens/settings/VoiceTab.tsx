import { useEffect, useState } from "react";
import { listVoices, speak, speechAvailable, stopSpeaking } from "../../services/speech";
import { useSettings } from "../../state/settings";
import { Toggle } from "./common";

function useVoices(): SpeechSynthesisVoice[] {
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>(() => listVoices());
  useEffect(() => {
    if (!speechAvailable()) return;
    const update = () => setVoices(listVoices());
    window.speechSynthesis.addEventListener("voiceschanged", update);
    return () => window.speechSynthesis.removeEventListener("voiceschanged", update);
  }, []);
  return voices;
}

export function VoiceTab() {
  const s = useSettings((st) => st.snapshot?.settings);
  const update = useSettings((st) => st.update);
  const voices = useVoices();
  if (!s) return null;

  return (
    <div className="tab">
      <h2>Voice</h2>
      <p className="setting-desc">
        Talk to Wisp with push-to-talk: press the voice shortcut, speak, and press it again. Wisp
        never listens in the background. Speech is transcribed by Gemini; answers are read aloud by
        Windows voices on this PC.
      </p>
      <section className="group">
        <Toggle
          id="voice-enabled"
          label="Allow voice conversation"
          checked={s.voiceEnabled}
          onChange={(voiceEnabled) => void update({ voiceEnabled })}
        />
        <Toggle
          id="speak-responses"
          label="Read typed answers aloud too"
          description="Voice conversations are always read aloud. Press Stop or Esc to interrupt."
          checked={s.speakResponses}
          onChange={(speakResponses) => void update({ speakResponses })}
        />
      </section>
      <section className="group">
        <h3>Reading voice</h3>
        {!speechAvailable() ? (
          <p className="muted">Text-to-speech isn't available here.</p>
        ) : (
          <>
            <label htmlFor="tts-voice">Voice</label>
            <select
              id="tts-voice"
              value={s.ttsVoice}
              onChange={(e) => void update({ ttsVoice: e.target.value })}
            >
              <option value="">System default</option>
              {voices.map((v) => (
                <option key={v.voiceURI} value={v.name}>
                  {v.name} ({v.lang})
                </option>
              ))}
            </select>
            <label htmlFor="tts-rate">Speed: {s.ttsRate.toFixed(1)}×</label>
            <input
              id="tts-rate"
              type="range"
              min={0.5}
              max={2}
              step={0.1}
              value={s.ttsRate}
              onChange={(e) => void update({ ttsRate: Number(e.target.value) })}
            />
            <div className="row-start">
              <button
                className="btn"
                onClick={() =>
                  void speak("Hi, I'm Wisp. This is how I sound.", {
                    voice: s.ttsVoice || undefined,
                    rate: s.ttsRate,
                  })
                }
              >
                Test voice
              </button>
              <button className="btn btn-ghost" onClick={stopSpeaking}>
                Stop
              </button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
