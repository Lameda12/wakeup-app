//! The 10 Hz loop: idle time + camera → engine → windows, sound, tray.

use std::time::{Duration, Instant};

use tauri::Emitter;
use wakeup_core::{AlarmReason, Camera, Event, Frame, Input, State, pick_phrase};

use crate::state::{AlarmView, Ctx, NudgeView, wall_ms};
use crate::{tray, windows};

const TICK: Duration = Duration::from_millis(100);
const SLOW_EVERY: Duration = Duration::from_secs(1);
const CAMERA_RETRY: Duration = Duration::from_secs(30);

/// Debug knobs for headless smoke tests and dogfooding:
/// `WAKEUP_NO_CAMERA=1` never opens the camera; `WAKEUP_TEST_ALARM_AFTER=5`
/// fires a test alarm 5s after launch.
struct DebugEnv {
    no_camera: bool,
    test_alarm_after: Option<f64>,
}

impl DebugEnv {
    fn read() -> Self {
        Self {
            no_camera: std::env::var_os("WAKEUP_NO_CAMERA").is_some(),
            test_alarm_after: std::env::var("WAKEUP_TEST_ALARM_AFTER")
                .ok()
                .and_then(|v| v.parse().ok()),
        }
    }
}

pub fn spawn(ctx: Ctx) {
    std::thread::Builder::new()
        .name("monitor".into())
        .spawn(move || run(ctx))
        .expect("spawn monitor thread");
}

fn run(ctx: Ctx) {
    let debug = DebugEnv::read();
    let mut eyes = Eyes::new();
    let mut test_fired = false;
    let mut last_label = String::new();

    loop {
        let tick_started = Instant::now();
        let mode = ctx.lock().engine.camera();
        let frame = if debug.no_camera {
            None
        } else {
            eyes.frame(mode)
        };
        let idle = wakeup_platform::idle_seconds();

        let (events, state, ear_closed) = {
            let mut shared = ctx.lock();
            let now = shared.now();
            let mut events = shared.engine.tick(Input { now, idle, frame });
            if debug.test_alarm_after.is_some_and(|after| now >= after) && !test_fired {
                test_fired = true;
                events.extend(shared.engine.trigger(now, AlarmReason::EyesClosed));
            }
            (
                events,
                shared.engine.state(),
                shared.engine.config().ear_closed,
            )
        };
        apply(&ctx, events);

        if matches!(state, State::Alarm { .. }) {
            let open = match frame {
                Some(Frame::Face(f)) => Some(if f.ear < ear_closed { "closed" } else { "open" }),
                _ => None,
            };
            let _ = ctx.app.emit("eyes", open);
        }

        let label = tray::status_label(&state, ctx.lock().now());
        if label != last_label {
            let _ = ctx.status_item.set_text(&label);
            last_label = label;
        }

        if let Some(rest) = TICK.checked_sub(tick_started.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// Owns the camera and face pipeline, opening and closing them as the
/// engine's camera mode changes.
struct Eyes {
    camera: Option<wakeup_vision::camera::Camera>,
    pipeline: Option<wakeup_vision::FacePipeline>,
    retry_at: Instant,
    last_slow: Instant,
}

impl Eyes {
    fn new() -> Self {
        Self {
            camera: None,
            pipeline: None,
            retry_at: Instant::now(),
            last_slow: Instant::now() - SLOW_EVERY,
        }
    }

    fn frame(&mut self, mode: Camera) -> Option<Frame> {
        match mode {
            Camera::Off => {
                // Dropping the camera stops the stream and the green light.
                self.camera = None;
                if let Some(p) = self.pipeline.as_mut() {
                    p.reset();
                }
                None
            }
            Camera::Slow if self.last_slow.elapsed() < SLOW_EVERY => None,
            Camera::Slow | Camera::On => {
                self.last_slow = Instant::now();
                self.capture()
            }
        }
    }

    fn capture(&mut self) -> Option<Frame> {
        if self.camera.is_none() {
            if Instant::now() < self.retry_at {
                return None;
            }
            let opened = wakeup_vision::camera::request_permission()
                .and_then(|()| wakeup_vision::camera::Camera::open(0, 640, 480));
            match opened {
                Ok(cam) => self.camera = Some(cam),
                Err(e) => {
                    eprintln!("[camera] {e:#}; retrying in {}s", CAMERA_RETRY.as_secs());
                    self.retry_at = Instant::now() + CAMERA_RETRY;
                    return None;
                }
            }
        }
        if self.pipeline.is_none() {
            match wakeup_vision::FacePipeline::new() {
                Ok(p) => self.pipeline = Some(p),
                Err(e) => {
                    eprintln!("[vision] {e:#}");
                    return None;
                }
            }
        }
        let (camera, pipeline) = (self.camera.as_mut()?, self.pipeline.as_mut()?);
        let result = camera.frame().and_then(|img| pipeline.process(&img));
        match result {
            Ok(r) => Some(r.sample.map_or(Frame::NoFace, |s| {
                Frame::Face(wakeup_core::Face {
                    ear: s.ear(),
                    pitch: s.pitch_deg,
                    roll: s.roll_deg,
                })
            })),
            Err(e) => {
                eprintln!("[camera] {e:#}; reopening");
                self.camera = None;
                None
            }
        }
    }
}

/// Turns engine events into windows, sound, volume and log lines.
pub fn apply(ctx: &Ctx, events: Vec<Event>) {
    for event in events {
        match event {
            Event::NudgeShown => {
                let view = NudgeView {
                    timeout_secs: ctx.lock().engine.config().nudge_timeout,
                    shown_at_ms: wall_ms(),
                };
                ctx.lock().nudge = Some(view.clone());
                let _ = ctx.app.emit("nudge", Some(view));
                windows::show_nudge(&ctx.app);
                ctx.audio.chime();
            }
            Event::NudgeCleared => {
                ctx.lock().nudge = None;
                let _ = ctx.app.emit("nudge", None::<NudgeView>);
                windows::hide_nudge(&ctx.app);
            }
            Event::AlarmStarted(reason) => {
                let started_at_ms = wall_ms();
                let view = AlarmView {
                    reason,
                    phrase: pick_phrase(started_at_ms / 1000).to_string(),
                    started_at_ms,
                };
                // Volume first so the very first beep is audible.
                let volume = wakeup_platform::volume::max_volume();
                {
                    let mut shared = ctx.lock();
                    shared.alarm = Some(view.clone());
                    shared.volume = volume;
                }
                ctx.audio.alarm();
                eprintln!("[alarm] {reason:?}, phrase: {:?}", view.phrase);
                let _ = ctx.app.emit("alarm", Some(view));
                windows::show_alarm(&ctx.app);
                ctx.log.alarm(reason);
            }
            Event::AlarmDismissed { reason, after } => {
                ctx.audio.stop();
                let volume = {
                    let mut shared = ctx.lock();
                    shared.alarm = None;
                    shared.volume.take()
                };
                drop(volume); // restores the volume, outside the lock
                eprintln!("[alarm] dismissed after {after:.1}s");
                let _ = ctx.app.emit("alarm", None::<AlarmView>);
                windows::hide_alarm(&ctx.app);
                ctx.log.dismissed(reason, after);
            }
            Event::Paused(_) | Event::Resumed => {}
        }
    }
}
