//! Scenario tests: simulated people at 10 fps against the real rules.

use crate::*;

const FPS: f64 = 10.0;
const OPEN: Face = Face {
    ear: 0.30,
    pitch: 10.0,
    roll: 0.0,
};
const CLOSED: Face = Face {
    ear: 0.08,
    pitch: 10.0,
    roll: 0.0,
};
const DOWN: Face = Face {
    ear: 0.25,
    pitch: 40.0,
    roll: 0.0,
};

/// A person at a desk. Time advances in 0.1s ticks; the camera only delivers
/// frames when the engine asks for it, like the real app.
struct Sim {
    engine: Engine,
    t: f64,
    last_input: f64,
    idle_supported: bool,
    camera_works: bool,
    events: Vec<(f64, Event)>,
}

impl Sim {
    fn new() -> Self {
        Self::with(Config::default())
    }

    fn with(cfg: Config) -> Self {
        Self {
            engine: Engine::new(cfg),
            t: 0.0,
            last_input: 0.0,
            idle_supported: true,
            camera_works: true,
            events: vec![],
        }
    }

    /// Runs for `secs`, showing the camera `face(t)` (None = no face in view).
    fn run(&mut self, secs: f64, face: impl Fn(f64) -> Option<Face>) -> &mut Self {
        let end = self.t + secs;
        while self.t < end - 1e-9 {
            self.t = ((self.t + 1.0 / FPS) * FPS).round() / FPS;
            let frame = match self.engine.camera() {
                Camera::Off => None,
                _ if !self.camera_works => None,
                // Slow mode: one frame a second.
                Camera::Slow if !((self.t * FPS).round() as u64).is_multiple_of(FPS as u64) => None,
                _ => Some(face(self.t).map_or(Frame::NoFace, Frame::Face)),
            };
            let idle = self.idle_supported.then_some(self.t - self.last_input);
            for e in self.engine.tick(Input {
                now: self.t,
                idle,
                frame,
            }) {
                self.events.push((self.t, e));
            }
        }
        self
    }

    /// Runs `secs` while typing continuously (looking at the screen).
    fn typing(&mut self, secs: f64) -> &mut Self {
        let end = self.t + secs;
        while self.t < end - 1e-9 {
            self.last_input = self.t;
            self.run(0.1, |_| Some(OPEN));
        }
        self
    }

    fn key(&mut self) -> &mut Self {
        self.last_input = self.t;
        self
    }

    fn state(&self) -> State {
        self.engine.state()
    }

    fn alarms(&self) -> Vec<(f64, AlarmReason)> {
        self.events
            .iter()
            .filter_map(|(t, e)| match e {
                Event::AlarmStarted(r) => Some((*t, *r)),
                _ => None,
            })
            .collect()
    }

    fn first_alarm(&self) -> (f64, AlarmReason) {
        *self
            .alarms()
            .first()
            .unwrap_or_else(|| panic!("no alarm; events: {:?}", self.events))
    }

    fn has(&self, event: Event) -> bool {
        self.events.iter().any(|(_, e)| *e == event)
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.25
}

#[test]
fn typing_keeps_camera_off() {
    let mut s = Sim::new();
    s.typing(300.0);
    assert_eq!(s.state(), State::Active);
    assert_eq!(s.engine.camera(), Camera::Off);
    assert!(s.events.is_empty());
}

#[test]
fn camera_turns_on_after_gate() {
    let mut s = Sim::new();
    s.typing(5.0).run(7.5, |_| Some(OPEN));
    assert_eq!(s.engine.camera(), Camera::Off);
    s.run(1.0, |_| Some(OPEN));
    assert_eq!(s.state(), State::Watching);
    assert_eq!(s.engine.camera(), Camera::On);
    s.key().run(0.1, |_| Some(OPEN));
    assert_eq!(s.engine.camera(), Camera::Off);
}

#[test]
fn dozing_off_fires_after_gate_plus_window() {
    let mut s = Sim::new();
    // Stop typing at t=10 and nod off at once: camera wakes at 18, alarm at 20.5.
    s.typing(10.0).run(20.0, |_| Some(CLOSED));
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::EyesClosed);
    assert!(near(t, 10.0 + 8.0 + 2.5), "fired at {t}");
    assert_eq!(
        s.engine.camera(),
        Camera::On,
        "camera stays on to verify the dismiss"
    );
}

#[test]
fn blinking_never_fires() {
    let mut s = Sim::new();
    // Blink for 0.4s every 3s while reading for 80s.
    s.typing(1.0)
        .run(80.0, |t| Some(if t % 3.0 < 0.4 { CLOSED } else { OPEN }));
    assert!(s.alarms().is_empty(), "{:?}", s.alarms());
}

#[test]
fn slow_blinks_and_squints_below_window_dont_fire() {
    let mut s = Sim::new();
    // Eyes shut 2.0s out of every 4s: drowsy-ish but never 2.5s straight.
    s.typing(1.0)
        .run(60.0, |t| Some(if t % 4.0 < 2.0 { CLOSED } else { OPEN }));
    assert!(s.alarms().is_empty(), "{:?}", s.alarms());
}

#[test]
fn eyes_closed_with_noisy_frames_still_fires() {
    let mut s = Sim::new();
    // Every 8th frame misreads as open.
    s.typing(1.0).run(20.0, |t| {
        Some(if ((t * FPS).round() as u64).is_multiple_of(8) {
            OPEN
        } else {
            CLOSED
        })
    });
    assert_eq!(s.first_alarm().1, AlarmReason::EyesClosed);
}

#[test]
fn head_down_at_phone_fires() {
    let mut s = Sim::new();
    s.typing(1.0).run(20.0, |_| Some(DOWN));
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::HeadDown);
    assert!(near(t, 1.0 + 8.0 + 3.0), "fired at {t}");
}

#[test]
fn reading_gets_a_nudge_then_alarm_if_ignored() {
    let mut s = Sim::new();
    s.typing(1.0).run(89.0, |_| Some(OPEN));
    assert!(!s.has(Event::NudgeShown));
    s.run(1.5, |_| Some(OPEN));
    assert!(s.has(Event::NudgeShown));
    assert!(matches!(s.state(), State::Nudge { .. }));
    s.run(10.0, |_| Some(OPEN));
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::Idle);
    assert!(near(t, 1.0 + 90.0 + 10.0), "fired at {t}");
}

#[test]
fn answering_the_nudge_clears_it() {
    let mut s = Sim::new();
    s.typing(1.0).run(95.0, |_| Some(OPEN));
    assert!(matches!(s.state(), State::Nudge { .. }));
    s.key().run(0.1, |_| Some(OPEN));
    assert!(s.has(Event::NudgeCleared));
    assert_eq!(s.state(), State::Active);
    assert!(s.alarms().is_empty());
}

#[test]
fn dozing_during_nudge_escalates_immediately() {
    let mut s = Sim::new();
    s.typing(1.0).run(92.0, |_| Some(OPEN));
    assert!(matches!(s.state(), State::Nudge { .. }));
    s.run(5.0, |_| Some(CLOSED));
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::EyesClosed);
    assert!(
        t < 1.0 + 92.0 + 3.0,
        "should not wait for the nudge timeout, fired at {t}"
    );
}

#[test]
fn walking_away_never_alarms() {
    let mut s = Sim::new();
    s.typing(1.0).run(600.0, |_| None);
    assert!(s.alarms().is_empty(), "{:?}", s.alarms());
    assert_eq!(s.state(), State::Away);
    assert_eq!(s.engine.camera(), Camera::Slow);
}

#[test]
fn coming_back_resumes_watching() {
    let mut s = Sim::new();
    s.typing(1.0).run(60.0, |_| None);
    assert_eq!(s.state(), State::Away);
    s.run(1.5, |_| Some(OPEN));
    assert_eq!(s.state(), State::Watching);
    // Sits back down and dozes off without touching the keyboard.
    s.run(5.0, |_| Some(CLOSED));
    assert_eq!(s.first_alarm().1, AlarmReason::EyesClosed);
}

#[test]
fn leaving_mid_nudge_goes_away_quietly() {
    let mut s = Sim::new();
    s.typing(1.0).run(92.0, |_| Some(OPEN));
    s.run(30.0, |_| None);
    assert!(s.has(Event::NudgeCleared));
    assert_eq!(s.state(), State::Away);
    assert!(s.alarms().is_empty());
}

#[test]
fn head_hitting_the_desk_is_not_away() {
    let mut s = Sim::new();
    // Watching, eyes drooping for 2s (under the 2.5s rule), then the face
    // turns sideways onto the desk and the detector loses it.
    s.typing(1.0)
        .run(9.0, |_| Some(OPEN))
        .run(2.0, |_| Some(CLOSED));
    let lost_at = s.t;
    s.run(10.0, |_| None);
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::LostWhileDrowsy);
    assert!(
        near(t, lost_at + 3.0),
        "fired at {t}, face lost at {lost_at}"
    );
}

#[test]
fn rolling_sideways_then_lost_is_drowsy() {
    let mut s = Sim::new();
    let tilted = Face { roll: 50.0, ..OPEN };
    s.typing(1.0)
        .run(9.0, |_| Some(OPEN))
        .run(1.5, |_| Some(tilted))
        .run(10.0, |_| None);
    assert_eq!(s.first_alarm().1, AlarmReason::LostWhileDrowsy);
}

#[test]
fn camera_on_with_no_face_ever_is_away() {
    let mut s = Sim::new();
    s.typing(1.0).run(30.0, |_| None);
    assert_eq!(s.state(), State::Away);
    assert!(s.alarms().is_empty());
}

#[test]
fn typing_the_phrase_does_not_end_the_alarm() {
    let mut s = Sim::new();
    s.typing(1.0).run(12.0, |_| Some(CLOSED));
    assert!(matches!(s.state(), State::Alarm { .. }));
    s.typing(10.0);
    assert!(matches!(s.state(), State::Alarm { .. }));
}

#[test]
fn dismiss_needs_open_eyes_while_typing() {
    let mut s = Sim::new();
    s.typing(1.0).run(12.0, |_| Some(CLOSED));

    // Typed it with eyes half shut: rejected, alarm keeps going.
    s.engine.start_typing();
    s.run(4.0, |t| Some(if t % 1.0 < 0.5 { CLOSED } else { OPEN }));
    let err = s.engine.try_dismiss(s.t).unwrap_err();
    assert!(
        matches!(err, DismissError::EyesNotOpen { open_fraction } if (0.3..0.7).contains(&open_fraction)),
        "{err:?}"
    );
    assert!(matches!(s.state(), State::Alarm { .. }));

    // Again, eyes open.
    s.engine.start_typing();
    s.run(4.0, |_| Some(OPEN));
    let events = s.engine.try_dismiss(s.t).unwrap();
    assert!(
        matches!(events[..], [Event::AlarmDismissed { reason: AlarmReason::EyesClosed, after }] if after > 7.0)
    );
    assert_eq!(s.state(), State::Active);
}

#[test]
fn dismiss_works_without_a_camera() {
    let mut s = Sim::new();
    s.typing(1.0).run(110.0, |_| Some(OPEN));
    assert_eq!(s.first_alarm().1, AlarmReason::Idle);
    s.camera_works = false;
    s.engine.start_typing();
    s.run(3.0, |_| None);
    assert!(s.engine.try_dismiss(s.t).is_ok());
}

#[test]
fn dismiss_without_alarm_is_an_error() {
    let mut s = Sim::new();
    assert_eq!(s.engine.try_dismiss(1.0), Err(DismissError::NoAlarm));
}

#[test]
fn pause_silences_until_it_expires() {
    let mut s = Sim::new();
    s.typing(1.0);
    s.engine
        .pause(PauseReason::Manual, Some(s.t + 900.0))
        .unwrap();
    s.run(899.0, |_| Some(CLOSED));
    assert!(s.alarms().is_empty());
    assert_eq!(s.engine.camera(), Camera::Off);
    s.run(2.0, |_| Some(CLOSED));
    assert!(s.has(Event::Resumed));
    s.run(15.0, |_| Some(CLOSED));
    assert_eq!(s.first_alarm().1, AlarmReason::EyesClosed);
}

#[test]
fn meeting_pause_has_no_expiry_and_resumes_on_request() {
    let mut s = Sim::new();
    s.engine.pause(PauseReason::Meeting, None).unwrap();
    s.run(3600.0, |_| Some(CLOSED));
    assert!(s.alarms().is_empty());
    assert_eq!(s.engine.resume(), vec![Event::Resumed]);
    assert_eq!(s.state(), State::Active);
}

#[test]
fn cannot_pause_out_of_an_alarm() {
    let mut s = Sim::new();
    s.typing(1.0).run(12.0, |_| Some(CLOSED));
    assert_eq!(
        s.engine.pause(PauseReason::Manual, None),
        Err(PauseError::AlarmActive)
    );
}

#[test]
fn always_on_camera_skips_the_gate() {
    let mut s = Sim::with(Config {
        camera_gate: 0.0,
        ..Config::default()
    });
    s.typing(5.0);
    assert_eq!(s.engine.camera(), Camera::On);
    s.run(3.0, |_| Some(CLOSED));
    let (t, _) = s.first_alarm();
    assert!(near(t, 5.0 + 2.5), "fired at {t}");
}

#[test]
fn without_idle_support_camera_runs_and_face_rules_still_work() {
    let mut s = Sim::new();
    s.idle_supported = false;
    s.run(200.0, |t| Some(if t < 150.0 { OPEN } else { CLOSED }));
    assert!(!s.has(Event::NudgeShown), "no idle data, no idle nudge");
    let (t, reason) = s.first_alarm();
    assert_eq!(reason, AlarmReason::EyesClosed);
    assert!(near(t, 152.5), "fired at {t}");
}

#[test]
fn flaky_face_detection_does_not_fire_eye_rule() {
    let mut s = Sim::new();
    // Face found in only 1 of 3 frames (bad light), and those read closed.
    s.typing(1.0).run(20.0, |t| {
        ((t * FPS).round() as u64)
            .is_multiple_of(3)
            .then_some(CLOSED)
    });
    assert!(
        !s.alarms()
            .iter()
            .any(|(_, r)| *r == AlarmReason::EyesClosed),
        "{:?}",
        s.alarms()
    );
}

#[test]
fn just_under_the_window_does_not_fire() {
    let mut s = Sim::new();
    // Watching from t=9; eyes shut 2.3s, open 1s, repeatedly.
    s.typing(1.0).run(9.0, |_| Some(OPEN));
    s.run(40.0, |t| {
        Some(if (t - 10.0).rem_euclid(3.3) < 2.3 {
            CLOSED
        } else {
            OPEN
        })
    });
    assert!(s.alarms().is_empty(), "{:?}", s.alarms());
}

#[test]
fn stepping_out_briefly_during_nudge_still_alarms_on_return() {
    let mut s = Sim::new();
    s.typing(1.0).run(92.0, |_| Some(OPEN));
    // Leans out of view past the nudge timeout, but not long enough to be away.
    s.run(9.0, |_| None);
    assert!(s.alarms().is_empty());
    s.run(1.0, |_| Some(OPEN));
    assert_eq!(s.first_alarm().1, AlarmReason::Idle);
}

#[test]
fn stalled_camera_does_not_fire_on_old_frames() {
    let mut s = Sim::new();
    s.typing(1.0)
        .run(9.0, |_| Some(OPEN))
        .run(2.0, |_| Some(CLOSED));
    // Camera stops delivering frames; time keeps passing.
    s.camera_works = false;
    s.run(5.0, |_| Some(CLOSED));
    assert!(
        !s.alarms()
            .iter()
            .any(|(_, r)| *r == AlarmReason::EyesClosed),
        "{:?}",
        s.alarms()
    );
}

#[test]
fn without_a_working_camera_the_idle_rule_still_runs() {
    let mut s = Sim::new();
    s.camera_works = false;
    s.typing(1.0).run(89.5, |_| None);
    assert_ne!(
        s.state(),
        State::Away,
        "no frames at all means blind, not away"
    );
    s.run(1.0, |_| None);
    assert!(s.has(Event::NudgeShown));
    s.run(10.0, |_| None);
    assert_eq!(s.first_alarm().1, AlarmReason::Idle);
}

#[test]
fn camera_dying_mid_watch_does_not_fake_a_head_on_desk() {
    let mut s = Sim::new();
    s.typing(1.0)
        .run(9.0, |_| Some(OPEN))
        .run(2.0, |_| Some(CLOSED));
    s.camera_works = false;
    s.run(20.0, |_| None);
    assert!(s.alarms().is_empty(), "{:?}", s.alarms());
}

#[test]
fn test_alarm_can_be_triggered_and_dismissed() {
    let mut s = Sim::new();
    s.typing(2.0);
    assert_eq!(
        s.engine.trigger(s.t, AlarmReason::EyesClosed),
        vec![Event::AlarmStarted(AlarmReason::EyesClosed)]
    );
    assert!(matches!(s.state(), State::Alarm { .. }));
    assert!(
        s.engine.trigger(s.t, AlarmReason::HeadDown).is_empty(),
        "already ringing"
    );
    s.engine.start_typing();
    s.run(2.0, |_| Some(OPEN));
    assert!(s.engine.try_dismiss(s.t).is_ok());
}

#[test]
fn idle_rule_can_be_turned_off() {
    let mut s = Sim::with(Config {
        idle_nudge_after: 0.0,
        ..Config::default()
    });
    s.typing(1.0).run(600.0, |_| Some(OPEN));
    assert!(!s.has(Event::NudgeShown));
    s.run(5.0, |_| Some(CLOSED));
    assert_eq!(
        s.first_alarm().1,
        AlarmReason::EyesClosed,
        "face rules still on"
    );
}

#[test]
fn meeting_pauses_and_resumes() {
    let mut s = Sim::new();
    s.typing(1.0);
    assert_eq!(
        s.engine.set_external_pause(PauseReason::Meeting, true),
        vec![Event::Paused(PauseReason::Meeting)]
    );
    // Re-checking while still in the meeting is a no-op.
    assert!(
        s.engine
            .set_external_pause(PauseReason::Meeting, true)
            .is_empty()
    );
    s.run(600.0, |_| Some(CLOSED));
    assert!(s.alarms().is_empty());
    assert_eq!(
        s.engine.set_external_pause(PauseReason::Meeting, false),
        vec![Event::Resumed]
    );
    assert_eq!(s.state(), State::Active);
}

#[test]
fn meeting_ending_does_not_lift_a_manual_pause() {
    let mut s = Sim::new();
    s.engine.pause(PauseReason::Manual, Some(900.0)).unwrap();
    assert!(
        s.engine
            .set_external_pause(PauseReason::Meeting, true)
            .is_empty(),
        "already paused"
    );
    assert!(
        s.engine
            .set_external_pause(PauseReason::Meeting, false)
            .is_empty()
    );
    assert!(matches!(
        s.state(),
        State::Paused {
            reason: PauseReason::Manual,
            ..
        }
    ));
}

#[test]
fn meeting_cannot_silence_a_ringing_alarm() {
    let mut s = Sim::new();
    s.typing(1.0).run(12.0, |_| Some(CLOSED));
    assert!(
        s.engine
            .set_external_pause(PauseReason::Meeting, true)
            .is_empty()
    );
    assert!(matches!(s.state(), State::Alarm { .. }));
}

#[test]
fn schedule_and_meeting_hand_over() {
    let mut s = Sim::new();
    s.engine.set_external_pause(PauseReason::Schedule, true);
    s.engine.set_external_pause(PauseReason::Meeting, true);
    assert!(matches!(
        s.state(),
        State::Paused {
            reason: PauseReason::Schedule,
            ..
        }
    ));
    // Work hours start mid-meeting: schedule lifts, the next meeting check pauses again.
    assert_eq!(
        s.engine.set_external_pause(PauseReason::Schedule, false),
        vec![Event::Resumed]
    );
    assert_eq!(
        s.engine.set_external_pause(PauseReason::Meeting, true),
        vec![Event::Paused(PauseReason::Meeting)]
    );
}
