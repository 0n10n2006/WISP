// Voice activity detection: decides when you've started talking and when
// you've finished, from microphone loudness alone (nothing leaves the PC).
//
// The background-noise level is learned continuously, so a fan or a noisy
// room doesn't count as speech. Speech must last a moment before it counts,
// and "done" means a pause of `silenceMs` after real speech.

export interface VadOptions {
  /** Pause after speech that means "done talking". */
  silenceMs: number;
  /** Give up if nothing is said for this long. */
  noSpeechMs?: number;
  /** Speech must last this long before it counts (filters clicks and coughs). */
  minSpeechMs?: number;
}

export type VadEvent = "speech" | "end" | "no-speech";

export class VoiceActivity {
  private floor = 0.004;
  private speaking = false;
  private heardSpeech = false;
  private aboveSince: number | null = null;
  private belowSince: number | null = null;
  private startedAt: number | null = null;
  /** First and last moments of real speech (ms), for trimming. */
  speechStart: number | null = null;
  speechEnd: number | null = null;
  private done = false;

  constructor(private readonly opts: VadOptions) {}

  get threshold(): number {
    return Math.max(this.floor * 3.2, this.floor + 0.012, 0.018);
  }

  /** Feed one loudness reading (RMS, 0..1) taken at time `t` (ms). */
  feed(rms: number, t: number): VadEvent | null {
    if (this.done) return null;
    const minSpeech = this.opts.minSpeechMs ?? 120;
    const noSpeech = this.opts.noSpeechMs ?? 8000;
    if (this.startedAt === null) {
      // Nobody starts talking within a few ms of pressing, so the first
      // reading is the room.
      this.startedAt = t;
      this.floor = Math.min(Math.max(rms, 0.001), 0.05);
    }
    // Track the noise floor: follow quiet readings quickly, loud ones slowly
    // (a little faster in the first moments, while learning the room).
    const learning = t - this.startedAt < 200;
    const rate = rms < this.floor ? 0.25 : this.speaking ? 0.0005 : learning ? 0.15 : 0.02;
    this.floor += (rms - this.floor) * rate;
    this.floor = Math.min(Math.max(this.floor, 0.001), 0.2);

    const loud = rms > this.threshold;
    // A little hysteresis: once speaking, slightly quieter still counts.
    const quiet = rms < this.threshold * 0.75;
    if (loud) {
      this.belowSince = null;
      this.aboveSince ??= t;
      if (!this.speaking && t - this.aboveSince >= minSpeech) {
        this.speaking = true;
        if (!this.heardSpeech) {
          this.heardSpeech = true;
          this.speechStart = this.aboveSince;
          this.speechEnd = t;
          return "speech";
        }
      }
      if (this.speaking) this.speechEnd = t;
    } else {
      this.aboveSince = null;
      if (quiet && this.speaking) {
        this.belowSince ??= t;
        if (t - this.belowSince >= this.opts.silenceMs) {
          this.speaking = false;
          this.done = true;
          return "end";
        }
      }
    }
    if (!this.heardSpeech && t - this.startedAt >= noSpeech) {
      this.done = true;
      return "no-speech";
    }
    return null;
  }

  get heard(): boolean {
    return this.heardSpeech;
  }
}

/**
 * Cut leading/trailing silence (keeping a little padding) so the
 * transcription model only hears the speech.
 */
export function trimToSpeech(
  samples: Float32Array,
  sampleRate: number,
  startMs: number | null,
  endMs: number | null,
  padMs = 400,
  /** Quiet last syllables fall under the speech threshold: keep more after. */
  tailMs = 800,
): Float32Array {
  if (startMs === null || endMs === null) return samples;
  const from = Math.max(0, Math.floor(((startMs - padMs) / 1000) * sampleRate));
  const to = Math.min(samples.length, Math.ceil(((endMs + tailMs) / 1000) * sampleRate));
  return to > from ? samples.subarray(from, to) : samples;
}

/** Scale so the loudest sample reaches `peak` (quiet microphones get louder). */
export function normalize(samples: Float32Array, peak = 0.9): Float32Array {
  let max = 0;
  for (const v of samples) max = Math.max(max, Math.abs(v));
  if (max < 1e-4 || max >= peak) return samples;
  const gain = Math.min(peak / max, 8);
  const out = new Float32Array(samples.length);
  for (let i = 0; i < samples.length; i++) out[i] = samples[i]! * gain;
  return out;
}
