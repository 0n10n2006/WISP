// Microphone capture for dictation and voice. Push-to-talk only: the stream
// is opened on start and every track is stopped on stop/cancel.

import { normalize, trimToSpeech, VoiceActivity, type VadEvent, type VadOptions } from "../lib/vad";
import type { WispError } from "./errors";

export interface Recording {
  base64: string;
  mimeType: string;
  durationMs: number;
}

export interface RecorderOptions {
  /** Microphone to use; empty for the Windows default. */
  deviceId?: string;
  /** Detect when the speaker starts and stops talking. */
  vad?: VadOptions;
  onVad?: (event: VadEvent) => void;
}

export interface AudioRecorder {
  /** Resolves once the microphone is live. `onLevel` gets 0..1 loudness. */
  start(onLevel: (level: number) => void, options?: RecorderOptions): Promise<void>;
  stop(): Promise<Recording>;
  cancel(): void;
}

function micError(e: unknown): WispError {
  const name = e instanceof DOMException ? e.name : "";
  if (name === "NotAllowedError" || name === "SecurityError")
    return {
      code: "permission_denied",
      message:
        "Wisp can't use the microphone. In Windows Settings → Privacy & security → Microphone, turn on access for desktop apps, then try again.",
      retryable: false,
    };
  if (name === "NotFoundError" || name === "OverconstrainedError")
    return {
      code: "permission_denied",
      message: "No microphone was found. Connect one and try again.",
      retryable: false,
    };
  if (name === "NotReadableError")
    return {
      code: "permission_denied",
      message: "The microphone is busy in another app. Close it there and try again.",
      retryable: false,
    };
  return {
    code: "unsupported",
    message: "Microphone capture isn't available here.",
    retryable: false,
  };
}

function pickMime(): string {
  const options = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus", "audio/mp4"];
  if (typeof MediaRecorder === "undefined") return "";
  return options.find((m) => MediaRecorder.isTypeSupported(m)) ?? "";
}

/** 16-bit PCM WAV from mono float samples. */
export function encodeWav(samples: Float32Array, sampleRate: number): Uint8Array {
  const out = new DataView(new ArrayBuffer(44 + samples.length * 2));
  const str = (o: number, t: string) =>
    [...t].forEach((c, i) => out.setUint8(o + i, c.charCodeAt(0)));
  str(0, "RIFF");
  out.setUint32(4, 36 + samples.length * 2, true);
  str(8, "WAVE");
  str(12, "fmt ");
  out.setUint32(16, 16, true);
  out.setUint16(20, 1, true); // PCM
  out.setUint16(22, 1, true); // mono
  out.setUint32(24, sampleRate, true);
  out.setUint32(28, sampleRate * 2, true);
  out.setUint16(32, 2, true);
  out.setUint16(34, 16, true);
  str(36, "data");
  out.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    const v = Math.max(-1, Math.min(1, samples[i]!));
    out.setInt16(44 + i * 2, v < 0 ? v * 0x8000 : v * 0x7fff, true);
  }
  return new Uint8Array(out.buffer);
}

const WAV_RATE = 16_000;

/**
 * Gemini documents WAV/MP3/OGG/FLAC input, not WebM, so recordings are
 * re-encoded: 16 kHz mono, silence before/after the speech trimmed, and
 * quiet recordings brought up to a healthy level.
 */
async function toWav(
  blob: Blob,
  speechStart: number | null = null,
  speechEnd: number | null = null,
): Promise<Uint8Array | null> {
  try {
    const raw = await blob.arrayBuffer();
    const decoder = new AudioContext();
    let decoded: AudioBuffer;
    try {
      decoded = await decoder.decodeAudioData(raw);
    } finally {
      void decoder.close().catch(() => {});
    }
    const frames = Math.ceil(decoded.duration * WAV_RATE);
    const offline = new OfflineAudioContext(1, frames, WAV_RATE);
    const src = offline.createBufferSource();
    src.buffer = decoded;
    src.connect(offline.destination);
    src.start();
    const rendered = await offline.startRendering();
    const speech = trimToSpeech(rendered.getChannelData(0), WAV_RATE, speechStart, speechEnd);
    return encodeWav(normalize(speech), WAV_RATE);
  } catch {
    return null;
  }
}

function bytesToBase64(buf: Uint8Array): string {
  let bin = "";
  const chunk = 0x8000;
  for (let i = 0; i < buf.length; i += chunk) {
    bin += String.fromCharCode(...buf.subarray(i, i + chunk));
  }
  return btoa(bin);
}

export function createMediaRecorder(): AudioRecorder {
  let stream: MediaStream | null = null;
  let recorder: MediaRecorder | null = null;
  let ctx: AudioContext | null = null;
  let meter: ReturnType<typeof setInterval> | null = null;
  let chunks: Blob[] = [];
  let started = 0;
  let vad: VoiceActivity | null = null;
  /** Set by cancel(): a start still waiting for the microphone must give up. */
  let cancelled = false;

  const teardown = () => {
    if (meter) clearInterval(meter);
    meter = null;
    stream?.getTracks().forEach((t) => t.stop());
    void ctx?.close().catch(() => {});
    stream = null;
    ctx = null;
  };

  return {
    async start(onLevel, options = {}) {
      cancelled = false;
      if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
        throw micError(null);
      }
      const audio: MediaTrackConstraints = {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
        channelCount: 1,
      };
      if (options.deviceId) audio.deviceId = { exact: options.deviceId };
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio });
      } catch (e) {
        // A chosen microphone that's been unplugged: fall back to the default.
        if (options.deviceId && e instanceof DOMException && e.name === "OverconstrainedError") {
          delete audio.deviceId;
          try {
            stream = await navigator.mediaDevices.getUserMedia({ audio });
          } catch (e2) {
            throw micError(e2);
          }
        } else throw micError(e);
      }
      if (cancelled) {
        // Cancelled while the microphone was opening: release it right away.
        teardown();
        throw { code: "cancelled", message: "Cancelled", retryable: false };
      }
      vad = options.vad ? new VoiceActivity(options.vad) : null;
      const mime = pickMime();
      try {
        recorder = new MediaRecorder(stream, mime ? { mimeType: mime } : undefined);
        chunks = [];
        recorder.ondataavailable = (e) => {
          if (e.data.size) chunks.push(e.data);
        };
        recorder.start(250);
      } catch (e) {
        recorder = null;
        teardown();
        throw micError(e);
      }
      started = performance.now();
      try {
        ctx = new AudioContext();
        const analyser = ctx.createAnalyser();
        analyser.fftSize = 1024;
        ctx.createMediaStreamSource(stream).connect(analyser);
        const data = new Float32Array(analyser.fftSize);
        // A timer (not animation frames) so detection keeps running even
        // when the window isn't being painted.
        meter = setInterval(() => {
          analyser.getFloatTimeDomainData(data);
          let sum = 0;
          for (const v of data) sum += v * v;
          const rms = Math.sqrt(sum / data.length);
          onLevel(Math.min(1, rms * 4));
          const event = vad?.feed(rms, performance.now() - started);
          if (event) options.onVad?.(event);
        }, 40);
      } catch {
        /* level meter and detection are optional */
      }
    },
    stop() {
      return new Promise<Recording>((resolve, reject) => {
        const r = recorder;
        if (!r) return reject({ code: "invalid", message: "Not recording", retryable: false });
        const finish = async () => {
          const durationMs = performance.now() - started;
          const mimeType = (r.mimeType || "audio/webm").split(";")[0]!;
          const blob = new Blob(chunks, { type: mimeType });
          chunks = [];
          const span = vad ? [vad.speechStart, vad.speechEnd] : [null, null];
          teardown();
          const wav = await toWav(blob, span[0] ?? null, span[1] ?? null);
          resolve(
            wav
              ? { base64: bytesToBase64(wav), mimeType: "audio/wav", durationMs }
              : {
                  base64: bytesToBase64(new Uint8Array(await blob.arrayBuffer())),
                  mimeType,
                  durationMs,
                },
          );
        };
        recorder = null;
        // A recorder that already ended (e.g. the microphone was unplugged)
        // won't fire onstop again: finish with what was recorded.
        if (r.state === "inactive") void finish();
        else {
          r.onstop = () => void finish();
          r.stop();
        }
      });
    },
    cancel() {
      cancelled = true;
      if (recorder && recorder.state !== "inactive") {
        recorder.onstop = null;
        recorder.stop();
      }
      recorder = null;
      chunks = [];
      teardown();
    },
  };
}

/**
 * Deterministic recorder for the browser preview and tests. With
 * `speechMs`, it pretends the speaker talks for that long and then stops
 * (so end-of-speech detection can be exercised).
 */
export function createFakeRecorder(speechMs?: number): AudioRecorder {
  let timer: ReturnType<typeof setInterval> | null = null;
  let timeouts: ReturnType<typeof setTimeout>[] = [];
  let started = 0;
  const clear = () => {
    if (timer) clearInterval(timer);
    timeouts.forEach(clearTimeout);
    timeouts = [];
  };
  return {
    async start(onLevel, options = {}) {
      started = Date.now();
      let t = 0;
      timer = setInterval(() => onLevel(0.3 + 0.3 * Math.sin((t += 0.6))), 80);
      if (speechMs !== undefined && options.onVad) {
        const silence = options.vad?.silenceMs ?? 1200;
        timeouts.push(setTimeout(() => options.onVad?.("speech"), 150));
        timeouts.push(setTimeout(() => options.onVad?.("end"), speechMs + silence));
      }
    },
    async stop() {
      clear();
      return { base64: "AAAA", mimeType: "audio/webm", durationMs: Date.now() - started };
    },
    cancel() {
      clear();
    },
  };
}
