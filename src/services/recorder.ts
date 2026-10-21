// Microphone capture for dictation and voice. Push-to-talk only: the stream
// is opened on start and every track is stopped on stop/cancel.

import type { WispError } from "./errors";

export interface Recording {
  base64: string;
  mimeType: string;
  durationMs: number;
}

export interface AudioRecorder {
  /** Resolves once the microphone is live. `onLevel` gets 0..1 loudness. */
  start(onLevel: (level: number) => void): Promise<void>;
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

async function blobToBase64(blob: Blob): Promise<string> {
  const buf = new Uint8Array(await blob.arrayBuffer());
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
  let raf = 0;
  let chunks: Blob[] = [];
  let started = 0;

  const teardown = () => {
    cancelAnimationFrame(raf);
    stream?.getTracks().forEach((t) => t.stop());
    void ctx?.close().catch(() => {});
    stream = null;
    ctx = null;
  };

  return {
    async start(onLevel) {
      if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
        throw micError(null);
      }
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          audio: { echoCancellation: true, noiseSuppression: true, channelCount: 1 },
        });
      } catch (e) {
        throw micError(e);
      }
      const mime = pickMime();
      recorder = new MediaRecorder(stream, mime ? { mimeType: mime } : undefined);
      chunks = [];
      recorder.ondataavailable = (e) => {
        if (e.data.size) chunks.push(e.data);
      };
      recorder.start(250);
      started = performance.now();
      try {
        ctx = new AudioContext();
        const analyser = ctx.createAnalyser();
        analyser.fftSize = 512;
        ctx.createMediaStreamSource(stream).connect(analyser);
        const data = new Uint8Array(analyser.fftSize);
        const tick = () => {
          analyser.getByteTimeDomainData(data);
          let sum = 0;
          for (const v of data) sum += ((v - 128) / 128) ** 2;
          onLevel(Math.min(1, Math.sqrt(sum / data.length) * 4));
          raf = requestAnimationFrame(tick);
        };
        tick();
      } catch {
        /* level meter is optional */
      }
    },
    stop() {
      return new Promise<Recording>((resolve, reject) => {
        const r = recorder;
        if (!r) return reject({ code: "invalid", message: "Not recording", retryable: false });
        r.onstop = async () => {
          const durationMs = performance.now() - started;
          const mimeType = (r.mimeType || "audio/webm").split(";")[0]!;
          const blob = new Blob(chunks, { type: mimeType });
          chunks = [];
          teardown();
          resolve({ base64: await blobToBase64(blob), mimeType, durationMs });
        };
        r.stop();
        recorder = null;
      });
    },
    cancel() {
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

/** Deterministic recorder for the browser preview and tests. */
export function createFakeRecorder(): AudioRecorder {
  let timer: ReturnType<typeof setInterval> | null = null;
  let started = 0;
  return {
    async start(onLevel) {
      started = Date.now();
      let t = 0;
      timer = setInterval(() => onLevel(0.3 + 0.3 * Math.sin((t += 0.6))), 80);
    },
    async stop() {
      if (timer) clearInterval(timer);
      return { base64: "AAAA", mimeType: "audio/webm", durationMs: Date.now() - started };
    },
    cancel() {
      if (timer) clearInterval(timer);
    },
  };
}
