// The only file that talks to the Rust side. Outside Tauri (plain browser,
// for design work) it falls back to a fake alarm so every screen can be previewed.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AlarmReason = "EyesClosed" | "HeadDown" | "Idle" | "LostWhileDrowsy";

export interface AlarmInfo {
  reason: AlarmReason;
  phrase: string;
  startedAtMs: number;
}

export interface NudgeInfo {
  timeoutSecs: number;
  shownAtMs: number;
}

/** Eyes as the camera sees them right now; null when there's no face/camera. */
export type Eyes = "open" | "closed" | null;

export type SubmitResult = { ok: true } | { ok: false; error: string };

const inTauri = "__TAURI_INTERNALS__" in window;

const fake = {
  alarm: {
    reason: "EyesClosed" as AlarmReason,
    phrase: "my eyes are open and my phone is down",
    startedAtMs: Date.now() - 4000,
  },
  nudge: { timeoutSecs: 10, shownAtMs: Date.now() },
};

export const getAlarm = (): Promise<AlarmInfo | null> =>
  inTauri ? invoke("alarm_info") : Promise.resolve(fake.alarm);

export const getNudge = (): Promise<NudgeInfo | null> =>
  inTauri ? invoke("nudge_info") : Promise.resolve(fake.nudge);

export const typingStarted = (): Promise<void> =>
  inTauri ? invoke("typing_started") : Promise.resolve();

export async function submitPhrase(text: string): Promise<SubmitResult> {
  if (!inTauri) {
    return text.trim().toLowerCase() === fake.alarm.phrase
      ? { ok: true }
      : { ok: false, error: "that's not the phrase. try again." };
  }
  try {
    await invoke("submit_phrase", { text });
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

/** Subscribe to a backend event; returns an unsubscribe function. */
function on<T>(event: string, cb: (payload: T) => void): () => void {
  if (!inTauri) return () => {};
  let unlisten: UnlistenFn | undefined;
  let cancelled = false;
  listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (cancelled) fn();
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

export const onAlarm = (cb: (info: AlarmInfo | null) => void) => on("alarm", cb);
export const onNudge = (cb: (info: NudgeInfo | null) => void) => on("nudge", cb);

export function onEyes(cb: (eyes: Eyes) => void): () => void {
  if (inTauri) return on("eyes", cb);
  // Preview: blink every few seconds so the indicator can be seen changing.
  const id = setInterval(() => cb(Date.now() % 5000 < 600 ? "closed" : "open"), 200);
  return () => clearInterval(id);
}
