# wakeup: macOS app design proposal

Status: **draft, awaiting approval**. Nothing here is built yet.

Tags used below: `[Inference]` = reasoned, not confirmed. `[Unverified]` = needs a spike to confirm. `[Speculation]` = a guess to measure.

## 1. What v1 does

A menu bar app that watches for three things and gets loud until you're back:

| # | Trigger | Signal | Default |
|---|---------|--------|---------|
| 1 | eyes closed | webcam, eye openness | closed for 2.5s |
| 2 | head down at phone | webcam, head pitch | pitched down for 3s |
| 3 | zoned out | keyboard/trackpad idle, face present | 90s no input |

Dismiss only by typing a random phrase while the webcam sees your eyes open.

Out of scope for v1: gaze tracking, accounts, cloud sync, Windows, stats dashboards, AI anything.

## 2. Key idea: input activity gates the camera

You can't type or move the trackpad while asleep or scrolling your phone. So:

- **While you're using the keyboard/trackpad, the camera is off.** No green light, ~zero CPU.
- After **8s of no input**, the camera turns on and the face rules start.
- Any input turns it back off.

This keeps the camera light off most of the day, saves battery and makes the app easy to trust. Cost: detection starts ~8s after you stop typing, plus ~0.5-1s camera warm-up `[Inference]`. An "always on" toggle exists for people who want it.

## 3. State machine

```
ACTIVE ── idle ≥ 8s ──▶ WATCHING (camera on, ~10 fps)
  ▲                       │ eyes closed ≥ 2.5s ───────────▶ ALARM(eyes)
  │                       │ pitch down ≥ 3s ──────────────▶ ALARM(phone)
  │                       │ idle ≥ 90s, face present ─────▶ NUDGE ── 10s no input ──▶ ALARM(idle)
  │                       │ no face ≥ 10s ────────────────▶ AWAY (camera 1 fps, no alarms)
  └──── any input ────────┘
ALARM ──▶ DISMISSING (phrase typed + eyes open) ──▶ COOLDOWN 30s ──▶ ACTIVE
PAUSED: manual pause, meeting detected, or outside active hours
```

Design calls in there:
- **AWAY:** walking away from your desk never fires the alarm. Otherwise it rings in an empty room.
- **NUDGE:** rule 3 shows a quiet "still here?" prompt first, because reading or watching something for 90s is normal. Rules 1 and 2 go straight to ALARM. (Open question Q3.)
- **Meetings:** auto-pause while another app is using the mic (CoreAudio "device is running somewhere"), so it never goes off mid-Zoom.

## 4. Detection

**Eyes closed.** Apple Vision `VNDetectFaceLandmarksRequest` gives eye contour points. Compute eye aspect ratio (EAR) per eye, average both. "Closed" = EAR below a per-user threshold for ≥ 80% of frames in a 2.5s window. Normal blinks (~0.1-0.4s) can't trigger it. The threshold comes from a 15s calibration in onboarding (5s eyes open, 3s eyes closed, take the midpoint). Glasses and low light need testing on real hardware `[Unverified]`.

**Head down.** `VNFaceObservation.pitch` (macOS 12+ `[Unverified]`). Laptop cameras sit below eye level, so "neutral" isn't 0°: calibration records your neutral pitch and the rule fires at neutral + 25° for 3s. If the face slides out the bottom of the frame while pitch was trending down, count it as head down, not AWAY.

**Idle.** `CGEventSource.secondsSinceLastEventType(.combinedSessionState, ...)` polled at 1 Hz. Reads system idle time without Accessibility or Input Monitoring permission `[Inference, verify in spike]`.

**No face / bad light / camera busy:** face rules switch off and only rule 3 runs. The menu bar icon shows a dimmed state so you know.

Why Vision and not MediaPipe: built in, on-device, no model files to ship, runs well on Apple Silicon `[Inference]`. If Vision EAR turns out too noisy in the spike, MediaPipe Face Landmarker (it has blink blendshapes) is the fallback.

## 5. Alarm and dismiss

- **Overlay:** full-screen `NSPanel` on every display, `.screenSaver` level, joins all Spaces and full-screen apps. Red flashing frame, big text: what triggered it.
- **Sound, escalating:** 0s tone at 60% volume → 5s siren at 100% → keeps going. System output volume is saved, set to max and unmuted via CoreAudio, then restored after dismiss `[Inference: no permission needed]`.
- **Dismiss:** show a random phrase ("i am awake and i will finish this"). You have to type it exactly while the webcam sees your eyes open for ≥ 90% of the typing time. If the camera is unavailable, the phrase alone works.
- No anti-quit tricks. Force Quit always works. It's your machine.

## 6. Privacy

All processing on-device. No frames saved, nothing sent anywhere. Only stored data: settings plus a local event log (trigger type, time, seconds to dismiss) for "you dozed 3x today" in the menu and for tuning thresholds. One permission: Camera.

## 7. Stack and structure

Swift 6 + SwiftUI, `MenuBarExtra`, macOS 14+ target, Apple Silicon first.

```
Wakeup/
  WakeupApp.swift              menu bar entry, app lifecycle
  Core/
    ActivityMonitor.swift      idle seconds (1 Hz)
    CameraPipeline.swift       AVCaptureSession 640x480 @ 10 fps, start/stop on gate
    FaceAnalyzer.swift         Vision → FaceSample { ear, pitch, facePresent, bbox }
    WakeEngine.swift           pure state machine: (time, idle, FaceSample?) → state + events
    MeetingDetector.swift      mic-in-use check
  Alarm/
    AlarmController.swift      escalation timeline
    OverlayPanel.swift
    DismissChallengeView.swift
    VolumeOverride.swift       CoreAudio save/set/restore
  Onboarding/                  camera permission, calibration
  Settings/                    thresholds, active hours, sound, pause 15/30/60
  Support/EventLog.swift       local JSON/SQLite
WakeupTests/
  WakeEngineTests.swift        synthetic signal streams → expected alarms
```

`WakeEngine` is pure (no camera, no clock), so every rule and edge case gets unit tests by feeding fake streams: blinks, a 2.5s closure, walk-away, reading for 2 min, etc.

**Build constraint:** this needs macOS + Xcode. I can write the code from this cloud session (Linux) but can't compile or run it here. You build and test on your M2.

Performance target: < 1% CPU in ACTIVE, < 8% in WATCHING on an M2 `[Speculation, measure in M0]`.

## 8. Distribution

Recommended: direct download. Developer ID signing + notarization (needs the $99/yr Apple Developer account), DMG, Sparkle for updates, license key via Lemon Squeezy or Stripe for the $9 early access. Mac App Store adds sandbox and review friction for an app that overrides volume and draws over everything `[Inference]`.

## 9. Milestones

| | Scope | Done when |
|---|---|---|
| **M0 spike** (1-2 days) | menu bar app, idle polling, camera + Vision printing EAR and pitch | thresholds look separable on you, with and without glasses, in low light |
| **M1 core** (week 1) | WakeEngine + tests, overlay, sound, phrase dismiss | you dogfood it for a full workday |
| **M2 product** (week 2) | onboarding + calibration, settings, pause, meeting detection, launch at login | a friend installs it with no help |
| **M3 ship** | signing, notarization, DMG, Sparkle, license key | download link works from your existing waitlist page |

## 10. Open questions

1. Native Swift (recommended) or Tauri + TypeScript + MediaPipe so it stays closer to your stack?
2. Direct download + license key (recommended), Mac App Store, or free/open source?
3. Rule 3 (no typing 90s): soft "still here?" nudge first (recommended), or straight to the alarm as originally specced?
4. Camera only when idle (recommended) or always on?
