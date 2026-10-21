// Local text-to-speech using the Windows voices exposed to WebView2.
// Nothing is sent over the network.

import { useActivity } from "../state/activity";

export function speechAvailable(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window;
}

export function listVoices(): SpeechSynthesisVoice[] {
  return speechAvailable() ? window.speechSynthesis.getVoices() : [];
}

/** Strip Markdown so it reads naturally. Code blocks are summarised. */
export function toSpeakable(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, " (code omitted) ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}#{1,6}\s+/gm, "")
    .replace(/^\s*[-*•]\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/(\*\*|__)(.*?)\1/g, "$2")
    .replace(/(\*|_)(.*?)\1/g, "$2")
    .replace(/([.!?:;])?[ \t]*\n{2,}/g, (_m, p: string | undefined) => (p ? `${p} ` : ". "))
    .replace(/\s+/g, " ")
    .trim();
}

export interface SpeakOptions {
  voice?: string;
  rate?: number;
}

/** Speak text; resolves when finished or stopped. Only one utterance at a time. */
export function speak(text: string, opts: SpeakOptions = {}): Promise<void> {
  const setSpeaking = useActivity.getState().setSpeaking;
  if (!speechAvailable()) return Promise.resolve();
  const synth = window.speechSynthesis;
  synth.cancel();
  const clean = toSpeakable(text);
  if (!clean) return Promise.resolve();
  return new Promise((resolve) => {
    const u = new SpeechSynthesisUtterance(clean);
    const v = opts.voice ? synth.getVoices().find((x) => x.name === opts.voice) : undefined;
    if (v) u.voice = v;
    u.rate = opts.rate ?? 1;
    const done = () => {
      setSpeaking(false);
      resolve();
    };
    u.onend = done;
    u.onerror = done;
    setSpeaking(true);
    synth.speak(u);
  });
}

export function stopSpeaking(): void {
  if (speechAvailable()) window.speechSynthesis.cancel();
  useActivity.getState().setSpeaking(false);
}
