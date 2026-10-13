# wakeup: app design

Status: **approved 2026-10-13. M0 waiting on a run on your Mac; M1 built.**

Decisions: Rust core + Tauri, macOS first then Linux (GNOME, KDE, wlroots/tiling), camera only on when input is idle, "still here?" nudge before the idle alarm, direct download + Lemon Squeezy license keys. Linux test machine: decide at M4.

Tags: `[Inference]` = reasoned, not confirmed. `[Unverified]` = needs a spike to confirm. `[Speculation]` = a guess to measure.

## 1. What v1 does

A tray/menu bar app that watches for three things and gets loud until you're back:

| # | Trigger | Signal | Default |
|---|---------|--------|---------|
| 1 | eyes closed | webcam, eye openness | closed for 2.5s |
| 2 | head down at phone | webcam, head pitch | pitched down for 3s |
| 3 | zoned out | keyboard/trackpad idle, face present | 90s no input, then nudge, then alarm |

Dismiss only by typing a random phrase while the webcam sees your eyes open.

Out of scope for v1: Windows, gaze tracking, accounts, cloud sync, stats dashboards, AI anything.

## 2. Key idea: input activity gates the camera

You can't type or move the trackpad while asleep or scrolling your phone. So:

- **While you're using the keyboard/trackpad, the camera is off.** No camera light, near-zero CPU.
- After **8s of no input**, the camera turns on and the face rules start.
- Any input turns it back off.

Cost: detection starts ~8s after you stop typing, plus ~0.5-1s camera warm-up `[Inference]`. An "always on" toggle exists.

## 3. State machine

```
ACTIVE ── idle ≥ 8s ──▶ WATCHING (camera on, ~10 fps)
  ▲                       │ eyes closed ≥ 2.5s ───────────▶ ALARM(eyes)
  │                       │ pitch down ≥ 3s ──────────────▶ ALARM(phone)
  │                       │ idle ≥ 90s, face present ─────▶ NUDGE ── 10s no input ──▶ ALARM(idle)
  │                       │ no face ≥ 10s ────────────────▶ AWAY (camera 1 fps, no alarms)
  └──── any input ────────┘
ALARM ──▶ DISMISSING (phrase typed + eyes open) ──▶ ACTIVE
PAUSED: manual pause (15/30/60 min), meeting detected, or outside active hours
```

- **AWAY:** walking away from the desk never fires the alarm.
- **Blind camera ≠ away.** No frames at all (permission denied, camera busy or broken) means the face rules can't run, so only the idle rule does. Without this, a broken camera would read as an empty room and silence everything.
- **Streaks, not percentages.** "Eyes closed 2.5s" means a continuous run of closed frames, tolerating misread gaps under 0.3s. An earlier "80% of the last 2.5s" version fired after 2.0s and on 2s-on/2s-off squinting; tests caught it.
- No cooldown state after dismissing: you just typed, so you're ACTIVE, and the normal camera gate takes over.
- **Face lost mid-droop is not AWAY.** The detector can't find a face turned fully sideways (head down on the desk, cheek on the arm). Found in M0 testing: tracking follows a tilt up to ~60° within 2-3 frames, but at 90° the face is lost. So if the face disappears while idle and the last samples showed eyes closing, pitch dropping or roll growing, treat it as dozing, not AWAY.
- **NUDGE:** quiet "still here?" card with a soft chime. Any key or trackpad movement clears it.
- **Meetings:** auto-pause while another app is using the mic.

## 4. Architecture

One Rust workspace. Detection and rules are shared across platforms; only a thin platform layer differs.

```
crates/
  wakeup-core/        WakeEngine state machine + rules. Pure: (time, idle, FaceSample?) → state + events.
  wakeup-vision/      camera capture + ONNX face models → FaceSample { ear, pitch, face_present, bbox }
  wakeup-platform/    trait Platform { idle_secs, volume save/set/restore, mic_in_use, overlay hints }
    macos/  linux/
src-tauri/            Tauri 2 shell: tray, windows, wiring, updater, license check
ui/                   React + Vite + TS: overlay, nudge, dismiss, settings, onboarding/calibration
models/               face detector + face landmark .onnx files (+ licenses)
```

**Why Tauri:** compiled Rust binary that uses the OS web view (WKWebView on macOS, WebKitGTK on Linux), so the UI is TypeScript you already know, and one codebase covers both OSes. The UI is small (tray, overlay, settings, onboarding), so a web view is fine for it.

**UI:** React, since you know it. The overlay and dismiss screens are the only parts that need polish.

## 5. Detection

**Face models.** ONNX Runtime (`ort` crate) running MediaPipe's face detector + face landmark models (468 3D points), converted to ONNX (done: outputs match the TFLite originals to ~1e-4, see `models/README.md`). Same models on both OSes, Apache 2.0 licensed `[Unverified, check the model cards]`. On macOS, ONNX Runtime's CoreML backend can push it onto Apple's GPU/Neural Engine `[Inference]`. Fallback if this is too slow or noisy: Apple Vision on macOS behind the same `FaceSample` interface.

**Eyes closed.** Eye aspect ratio (EAR) from the eyelid landmarks, averaged over both eyes. "Closed" = EAR below your threshold for ≥ 80% of frames in a 2.5s window, so normal blinks (~0.1-0.4s) can't trigger it. The threshold comes from a 15s calibration in onboarding (5s eyes open, 3s eyes closed, take the midpoint).

**Head down.** Pitch from the 3D landmarks (face-plane normal). Laptop cameras sit below eye level, so calibration records your neutral pitch, and the rule fires at neutral + 25° for 3s. If the face slides out the bottom of the frame while pitch was trending down, that counts as head down, not AWAY.

**Camera.** `nokhwa` crate: AVFoundation on macOS, V4L2 on Linux. 640x480 at 10 fps.

**No face / bad light / camera busy:** face rules switch off and only the idle rule runs. The tray icon dims so you know.

## 6. Platform layer

| | macOS | Linux |
|---|---|---|
| Idle time | `CGEventSourceSecondsSinceLastEventType`, no extra permission `[Inference]` | GNOME: Mutter IdleMonitor over D-Bus. KDE, Hyprland, Sway: `ext-idle-notify-v1` Wayland protocol. X11: XScreenSaver. `[Unverified per desktop]` |
| Volume override | CoreAudio save → max + unmute → restore | PipeWire/PulseAudio (`libpulse`) |
| Mic in use (meeting) | CoreAudio "device running somewhere" | PulseAudio source outputs |
| Overlay above everything | `NSPanel` at screen-saver level on every display and Space | KDE + wlroots: layer-shell overlay layer. GNOME: no layer-shell, so a fullscreen window + critical notification, and the **sound carries the alarm** `[Inference]` |
| Tray | menu bar | StatusNotifierItem. Stock Fedora GNOME needs the AppIndicator extension; Ubuntu ships it on `[Inference]` |

GNOME is the weakest target: it can't guarantee the overlay covers a fullscreen app. The alarm still works because the sound is unmissable.

## 7. Alarm and dismiss

- **Overlay:** full-screen on every display, red flashing frame, big text saying what triggered it.
- **Sound, escalating:** 0s tone at 60% volume → 5s siren at 100% → keeps going. System volume is saved, maxed, unmuted, then restored after dismiss.
- **Dismiss:** type a random phrase ("i am awake and i will finish this") exactly, while the webcam sees your eyes open for ≥ 90% of the typing time. Camera unavailable: phrase alone works.
- No anti-quit tricks. Force Quit always works.

## 8. Privacy

All processing on-device. No frames saved, nothing sent anywhere except one license check. Stored locally: settings + an event log (trigger type, time, seconds to dismiss) for "you dozed 3x today" in the tray menu and for tuning thresholds. One permission on macOS: Camera.

## 9. Distribution

- **macOS:** Developer ID signing + notarization ($99/yr Apple Developer account), DMG, Tauri's built-in signed updater.
- **Linux:** AppImage + `.deb` + AUR package. No Flatpak in v1: its sandbox gets in the way of idle detection, volume and the camera `[Inference]`.
- **License:** key entered once, validated online, cached for offline use. Provider: open question below.
- **Builds:** GitHub Actions on macOS and Ubuntu runners build and sign releases, so you don't need to build releases locally.

**Where code gets tested:** this cloud session is Linux, so I can build and unit-test `wakeup-core` and the Linux pieces here (no camera or display). Anything touching the camera or macOS has to run on your M2.

Performance target: < 1% CPU in ACTIVE, < 8% in WATCHING on an M2 `[Speculation, measure in M0]`.

## 10. Milestones

| | Scope | Done when |
|---|---|---|
| **M0 spike** (2-3 days) | Rust CLI on your Mac: camera → ONNX landmarks → print EAR, pitch, idle | eyes-open vs closed and neutral vs head-down are clearly separable on you, with and without glasses, in low light |
| **M1 core** (week 1) | WakeEngine + unit tests, Tauri shell, tray, overlay, sound, nudge, phrase dismiss (macOS) | you dogfood it for a full workday |
| **M2 product** (week 2) | onboarding + calibration, settings, pause, meeting detection, launch at login | a friend installs it with no help |
| **M3 ship macOS** | signing, notarization, DMG, updater, license key, CI release | download link works from your waitlist page |
| **M4 Linux** | platform layer for GNOME, KDE, wlroots, X11; AppImage, .deb, AUR | alarm fires and dismisses on Ubuntu GNOME, KDE and Hyprland |

## 11. M0 status

Built: `wakeup-vision` (detector + face mesh + EAR/pitch/roll, frame-to-frame tracking), `wakeup-platform` (macOS idle time), `wakeup-spike` CLI with a guided calibration session. 15 tests, including end-to-end runs of the real models on a test face (open-eye EAR ~0.32, roll correct under mirror and 45° tilt, re-detects after the face leaves).

Not yet verified (needs your Mac): closed-eye EAR, head-down pitch sign and size, camera capture on macOS, idle time, CPU on an M2. That's what `wakeup-spike session` measures. See README.

## 12. M1 status

Built: `wakeup-core` engine (35 scenario tests), the Tauri tray app (10 Hz monitor loop, camera gating, full-screen alarm on every display at screen-saver level on macOS, synthesized escalating siren, volume override and restore, "still here?" nudge with chime, phrase dismiss verified against the camera, pause 15/60 min, test alarm, `config.json`, `events.jsonl`), and CI that produces an unsigned `wakeup.app`.

Verified here: the whole alarm flow runs in the real app on Linux under a virtual display (alarm fires, wrong text is highlighted, the correct phrase dismisses, events are logged). Not verified: anything on real macOS hardware, including window level over full-screen apps, the camera prompt from the .app bundle, and the volume override.

Known M1 limits: thresholds come from `config.json` (no settings screen or calibration UI yet), the nudge window may take keyboard focus on macOS, and Linux has no idle time yet (camera stays on, no idle rule) until M4.
