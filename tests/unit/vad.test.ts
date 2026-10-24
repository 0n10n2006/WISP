import { normalize, trimToSpeech, VoiceActivity } from "../../src/lib/vad";

/** Feed `ms` of a constant level every 40 ms, starting at `t`. Returns events and the end time. */
function run(vad: VoiceActivity, level: number, ms: number, t: number) {
  const events: string[] = [];
  for (let at = t; at < t + ms; at += 40) {
    const e = vad.feed(level, at);
    if (e) events.push(e);
  }
  return { events, t: t + ms };
}

describe("VoiceActivity", () => {
  it("hears speech, then ends after the pause", () => {
    const vad = new VoiceActivity({ silenceMs: 1000 });
    let r = run(vad, 0.003, 600, 0);
    expect(r.events).toEqual([]);
    r = run(vad, 0.2, 800, r.t);
    expect(r.events).toEqual(["speech"]);
    // A short breath doesn't end it.
    r = run(vad, 0.003, 500, r.t);
    expect(r.events).toEqual([]);
    r = run(vad, 0.2, 400, r.t);
    r = run(vad, 0.003, 1200, r.t);
    expect(r.events).toEqual(["end"]);
    expect(vad.speechStart).toBe(600);
    expect(vad.speechEnd).toBeGreaterThan(1800);
    // Finished: nothing more.
    expect(run(vad, 0.3, 400, r.t).events).toEqual([]);
  });

  it("ignores clicks and steady background noise", () => {
    const vad = new VoiceActivity({ silenceMs: 800, noSpeechMs: 4000 });
    // A loud room learned as the floor.
    let r = run(vad, 0.03, 2000, 0);
    // A 40 ms click.
    expect(vad.feed(0.5, r.t)).toBeNull();
    r = run(vad, 0.03, 2200, r.t + 40);
    expect(r.events).toEqual(["no-speech"]);
    expect(vad.heard).toBe(false);
  });

  it("gives up when nothing is said", () => {
    const vad = new VoiceActivity({ silenceMs: 1000, noSpeechMs: 3000 });
    expect(run(vad, 0.002, 3200, 0).events).toEqual(["no-speech"]);
  });
});

describe("audio clean-up", () => {
  it("trims to the speech with padding", () => {
    const samples = new Float32Array(16000 * 3); // 3 s at 16 kHz
    const out = trimToSpeech(samples, 16000, 1000, 2000, 250, 250);
    expect(out.length).toBe(16000 * 1.5);
    // By default more is kept after the speech than before it.
    expect(trimToSpeech(samples, 16000, 1000, 2000).length).toBe(16000 * 2.2);
    expect(trimToSpeech(samples, 16000, null, null).length).toBe(samples.length);
  });

  it("boosts quiet recordings, up to a limit", () => {
    const quiet = new Float32Array([0.1, -0.05, 0.02]);
    expect(Math.max(...normalize(quiet))).toBeCloseTo(0.8); // capped at 8×
    const normal = new Float32Array([0.3, -0.45]);
    expect(Math.min(...normalize(normal))).toBeCloseTo(-0.9);
    const silent = new Float32Array([0, 0]);
    expect(normalize(silent)).toBe(silent);
  });
});
