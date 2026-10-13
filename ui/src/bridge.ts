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

// ---------- settings & calibration ----------

export interface ActiveHours {
  start: string;
  end: string;
  /** ISO weekdays, 1 = Monday ... 7 = Sunday. */
  days: number[];
}

/** Mirrors the Rust `Settings` (engine thresholds are flattened in). */
export interface Settings {
  camera_gate: number;
  ear_closed: number;
  eyes_closed_for: number;
  pitch_neutral: number;
  pitch_down: number;
  head_down_for: number;
  idle_nudge_after: number;
  nudge_timeout: number;
  calibrated: boolean;
  onboarded: boolean;
  launch_at_login: boolean;
  meeting_pause: boolean;
  active_hours_enabled: boolean;
  active_hours: ActiveHours;
  /** Engine fields the UI doesn't edit, passed through untouched. */
  [other: string]: unknown;
}

export interface CalibrationResult {
  ear_closed: number;
  ear_open_typical: number;
  ear_shut_typical: number;
  pitch_neutral: number;
  pitch_down: number | null;
  warnings: string[];
}

export type Phase = "open" | "closed" | "down";
export interface CalibFrame {
  face: boolean;
  ear: number | null;
}

type Result<T> = { ok: true; value: T } | { ok: false; error: string };

async function call<T>(cmd: string, args?: Record<string, unknown>, fake?: () => T): Promise<Result<T>> {
  try {
    const value = inTauri ? await invoke<T>(cmd, args) : (fake ? fake() : (undefined as T));
    return { ok: true, value };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

let fakeSettings: Settings = {
  camera_gate: 8,
  ear_closed: 0.2,
  eyes_closed_for: 2.5,
  pitch_neutral: 10,
  pitch_down: 35,
  head_down_for: 3,
  idle_nudge_after: 90,
  nudge_timeout: 10,
  calibrated: true,
  onboarded: true,
  launch_at_login: true,
  meeting_pause: true,
  active_hours_enabled: true,
  active_hours: { start: "09:00", end: "18:00", days: [1, 2, 3, 4, 5] },
};

export const getSettings = () => call<Settings>("get_settings", undefined, () => fakeSettings);
export const saveSettings = (settings: Settings) =>
  call<Settings>("save_settings", { settings }, () => (fakeSettings = settings));

export const calibrationStart = () => call<void>("calibration_start");
export const calibrationPhase = (phase: Phase) => call<void>("calibration_phase", { phase });
export const calibrationFinish = () =>
  call<CalibrationResult>("calibration_finish", undefined, () => ({
    ear_closed: 0.19,
    ear_open_typical: 0.31,
    ear_shut_typical: 0.08,
    pitch_neutral: 9,
    pitch_down: 33,
    warnings: [],
  }));
export const calibrationCancel = () => call<void>("calibration_cancel");
export const finishOnboarding = (launchAtLogin: boolean) =>
  call<void>("finish_onboarding", { launchAtLogin });
export const testAlarm = () => call<void>("test_alarm");
export const openCalibration = () => call<void>("open_calibration");
export const chime = () => call<void>("chime");

export function onCalibFrame(cb: (frame: CalibFrame) => void): () => void {
  if (inTauri) return on("calib-frame", cb);
  const id = setInterval(() => cb({ face: true, ear: 0.28 + Math.random() * 0.04 }), 100);
  return () => clearInterval(id);
}

/** Hides the current window (settings/onboarding are reused, never closed). */
export async function hideWindow(): Promise<void> {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().hide();
}
