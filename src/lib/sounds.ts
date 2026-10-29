// Soft cues for voice: a rising note when Wisp starts listening, a falling
// one when it's done, a low one when it heard nothing. Made on the fly with
// Web Audio (no sound files), very quiet, and skipped where audio isn't there.

let ctx: AudioContext | null = null;

function audio(): AudioContext | null {
  if (typeof window === "undefined" || typeof window.AudioContext !== "function") return null;
  try {
    ctx ??= new window.AudioContext();
    return ctx;
  } catch {
    return null;
  }
}

function tone(freqs: number[], step = 0.07, volume = 0.05) {
  const a = audio();
  if (!a) return;
  if (a.state === "suspended") void a.resume().catch(() => {});
  const t0 = a.currentTime + 0.01;
  freqs.forEach((f, i) => {
    const osc = a.createOscillator();
    const gain = a.createGain();
    osc.type = "sine";
    osc.frequency.value = f;
    const at = t0 + i * step;
    gain.gain.setValueAtTime(0, at);
    gain.gain.linearRampToValueAtTime(volume, at + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + step + 0.08);
    osc.connect(gain).connect(a.destination);
    osc.start(at);
    osc.stop(at + step + 0.1);
  });
}

export type Cue = "start" | "done" | "nothing";

export function playCue(cue: Cue) {
  try {
    if (cue === "start") tone([660, 880]);
    else if (cue === "done") tone([880, 660]);
    else tone([392], 0.12, 0.035);
  } catch {
    // Sound is a nicety; never let it break voice.
  }
}
