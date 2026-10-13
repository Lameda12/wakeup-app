use std::collections::VecDeque;

use crate::config::Config;

/// One face measurement from the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    /// Mean eye aspect ratio of both eyes.
    pub ear: f32,
    pub pitch: f32,
    pub roll: f32,
}

/// What a camera frame showed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Frame {
    NoFace,
    Face(Face),
}

/// Everything the engine learns in one tick. `now` is monotonic seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    pub now: f64,
    /// Seconds since the last keyboard/trackpad event, `None` if the OS can't tell us.
    pub idle: Option<f64>,
    /// `None` when the camera is off or produced no frame this tick.
    pub frame: Option<Frame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlarmReason {
    EyesClosed,
    HeadDown,
    /// Ignored the "still here?" nudge.
    Idle,
    /// Face vanished mid-droop: head on the desk, turned sideways.
    LostWhileDrowsy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PauseReason {
    Manual,
    Meeting,
    Schedule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    /// Using the keyboard/trackpad. Camera off.
    Active,
    /// Input idle: camera on, rules running.
    Watching,
    /// "Still here?" shown.
    Nudge {
        since: f64,
    },
    Alarm {
        reason: AlarmReason,
        since: f64,
    },
    /// Walked away. Camera slow, no alarms.
    Away,
    Paused {
        reason: PauseReason,
        until: Option<f64>,
    },
}

/// How the app should run the camera for the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Camera {
    Off,
    /// Just enough to notice you're back (~1 fps).
    Slow,
    /// Full rate (~10 fps).
    On,
}

/// Things the UI has to react to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    NudgeShown,
    NudgeCleared,
    AlarmStarted(AlarmReason),
    AlarmDismissed { reason: AlarmReason, after: f64 },
    Paused(PauseReason),
    Resumed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DismissError {
    NoAlarm,
    /// Eyes weren't open enough while typing. Start the phrase again.
    EyesNotOpen {
        open_fraction: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PauseError {
    /// Can't pause your way out of an alarm.
    AlarmActive,
}

/// Frames seen while the dismiss phrase is being typed.
#[derive(Debug, Clone, Copy, Default)]
struct Typing {
    frames: u32,
    eyes_open: u32,
}

pub struct Engine {
    cfg: Config,
    state: State,
    /// Recent frames, oldest first, trimmed to `cfg.history()`.
    frames: VecDeque<(f64, Frame)>,
    watching_since: f64,
    last_face_at: Option<f64>,
    /// Last frame of any kind. No recent frames means the camera is broken or
    /// busy: face rules can't run, so don't mistake it for an empty room.
    last_frame_at: Option<f64>,
    typing: Option<Typing>,
}

/// Seconds without any frame before the engine treats the camera as blind.
const BLIND_AFTER: f64 = 2.0;

impl Engine {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            state: State::Active,
            frames: VecDeque::new(),
            watching_since: 0.0,
            last_face_at: None,
            last_frame_at: None,
            typing: None,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// New thresholds apply from the next tick.
    pub fn set_config(&mut self, cfg: Config) {
        self.cfg = cfg;
    }

    pub fn camera(&self) -> Camera {
        match self.state {
            State::Active | State::Paused { .. } => Camera::Off,
            State::Away => Camera::Slow,
            State::Watching | State::Nudge { .. } | State::Alarm { .. } => Camera::On,
        }
    }

    pub fn tick(&mut self, input: Input) -> Vec<Event> {
        let now = input.now;
        if let Some(frame) = input.frame {
            self.record(now, frame);
        }

        match self.state {
            State::Paused { until, .. } => {
                if until.is_some_and(|u| now >= u) {
                    self.enter_active();
                    return vec![Event::Resumed];
                }
                vec![]
            }
            // Only a successful dismiss ends an alarm; typing the phrase is input too.
            State::Alarm { .. } => vec![],
            State::Active => {
                if input.idle.is_none_or(|idle| idle >= self.cfg.camera_gate) {
                    self.enter_watching(now);
                }
                vec![]
            }
            State::Away => {
                if self.user_active(input.idle) {
                    self.enter_active();
                } else if matches!(input.frame, Some(Frame::Face(_))) {
                    self.enter_watching(now);
                    self.last_face_at = Some(now);
                }
                vec![]
            }
            State::Watching | State::Nudge { .. } => self.watch(input),
        }
    }

    fn watch(&mut self, input: Input) -> Vec<Event> {
        let now = input.now;
        let nudge_since = match self.state {
            State::Nudge { since } => Some(since),
            _ => None,
        };
        let cleared = |events: &mut Vec<Event>| {
            if nudge_since.is_some() {
                events.push(Event::NudgeCleared);
            }
        };
        let mut events = vec![];

        // Input after the nudge appeared clears it, whatever the camera gate is.
        let answered_nudge = nudge_since
            .zip(input.idle)
            .is_some_and(|(since, idle)| idle < now - since);
        if answered_nudge || self.user_active(input.idle) {
            cleared(&mut events);
            self.enter_active();
            if self.cfg.camera_gate <= 0.0 {
                self.enter_watching(now);
            }
            return events;
        }

        if self.sustained(now, self.cfg.eyes_closed_for, |c, f| c.is_eye_closed(f.ear)) {
            return self.alarm(now, AlarmReason::EyesClosed, events);
        }
        if self.sustained(now, self.cfg.head_down_for, |c, f| c.is_head_down(f.pitch)) {
            return self.alarm(now, AlarmReason::HeadDown, events);
        }

        let face_now = matches!(self.frames.back(), Some((t, Frame::Face(_))) if *t >= now - 1e-9);
        let blind = self.last_frame_at.is_none_or(|t| now - t > BLIND_AFTER);
        if !face_now && !blind {
            let lost_at = self.last_face_at.unwrap_or(self.watching_since);
            let lost_for = now - lost_at;
            if lost_for >= self.cfg.lost_while_drowsy_after && self.was_drooping(lost_at) {
                return self.alarm(now, AlarmReason::LostWhileDrowsy, events);
            }
            if lost_for >= self.cfg.away_after {
                cleared(&mut events);
                self.state = State::Away;
                return events;
            }
        }

        match nudge_since {
            // Only if you're still in front of the screen; if you left, the
            // away rule above takes over instead.
            Some(since) if now - since >= self.cfg.nudge_timeout && (face_now || blind) => {
                return self.alarm(now, AlarmReason::Idle, events);
            }
            None if self.cfg.idle_nudge_after > 0.0
                && input
                    .idle
                    .is_some_and(|idle| idle >= self.cfg.idle_nudge_after) =>
            {
                self.state = State::Nudge { since: now };
                events.push(Event::NudgeShown);
            }
            _ => {}
        }
        events
    }

    /// Starts an alarm now, e.g. "test alarm" from the menu. No-op if one is
    /// already ringing.
    pub fn trigger(&mut self, now: f64, reason: AlarmReason) -> Vec<Event> {
        let mut events = vec![];
        match self.state {
            State::Alarm { .. } => return events,
            State::Nudge { .. } => events.push(Event::NudgeCleared),
            _ => {}
        }
        self.alarm(now, reason, events)
    }

    /// First keystroke of the dismiss phrase: start checking that eyes stay open.
    pub fn start_typing(&mut self) {
        if matches!(self.state, State::Alarm { .. }) {
            self.typing = Some(Typing::default());
        }
    }

    /// The phrase was typed correctly. Succeeds if eyes were open while typing
    /// (or the camera saw nothing at all, e.g. it's unavailable).
    pub fn try_dismiss(&mut self, now: f64) -> Result<Vec<Event>, DismissError> {
        let State::Alarm { reason, since } = self.state else {
            return Err(DismissError::NoAlarm);
        };
        let typing = self.typing.take().unwrap_or_default();
        if typing.frames > 0 {
            let open_fraction = typing.eyes_open as f32 / typing.frames as f32;
            if open_fraction < self.cfg.dismiss_eyes_open {
                return Err(DismissError::EyesNotOpen { open_fraction });
            }
        }
        self.enter_active();
        Ok(vec![Event::AlarmDismissed {
            reason,
            after: now - since,
        }])
    }

    pub fn pause(
        &mut self,
        reason: PauseReason,
        until: Option<f64>,
    ) -> Result<Vec<Event>, PauseError> {
        if matches!(self.state, State::Alarm { .. }) {
            return Err(PauseError::AlarmActive);
        }
        let mut events = vec![];
        if matches!(self.state, State::Nudge { .. }) {
            events.push(Event::NudgeCleared);
        }
        self.state = State::Paused { reason, until };
        self.frames.clear();
        events.push(Event::Paused(reason));
        Ok(events)
    }

    pub fn resume(&mut self) -> Vec<Event> {
        if matches!(self.state, State::Paused { .. }) {
            self.enter_active();
            return vec![Event::Resumed];
        }
        vec![]
    }

    /// For pauses driven by the outside world (a meeting started, active
    /// hours ended): call it whenever the condition is re-checked. Never
    /// overrides another pause or an alarm, and only lifts its own pause.
    pub fn set_external_pause(&mut self, reason: PauseReason, active: bool) -> Vec<Event> {
        match (self.state, active) {
            (
                State::Paused {
                    reason: current, ..
                },
                false,
            ) if current == reason => self.resume(),
            (State::Paused { .. } | State::Alarm { .. }, _) | (_, false) => vec![],
            (_, true) => self.pause(reason, None).unwrap_or_default(),
        }
    }

    fn record(&mut self, now: f64, frame: Frame) {
        self.last_frame_at = Some(now);
        if let Frame::Face(_) = frame {
            self.last_face_at = Some(now);
        }
        if let Some(typing) = self.typing.as_mut() {
            typing.frames += 1;
            if let Frame::Face(f) = frame {
                typing.eyes_open += u32::from(!self.cfg.is_eye_closed(f.ear));
            }
        }
        self.frames.push_back((now, frame));
        let horizon = now - self.history();
        while self.frames.front().is_some_and(|(t, _)| *t < horizon) {
            self.frames.pop_front();
        }
    }

    fn history(&self) -> f64 {
        self.cfg
            .history()
            .max(self.cfg.lost_while_drowsy_after + self.cfg.droop_lookback + 1.0)
    }

    fn user_active(&self, idle: Option<f64>) -> bool {
        idle.is_some_and(|idle| idle < self.cfg.camera_gate)
    }

    fn enter_active(&mut self) {
        self.state = State::Active;
        self.frames.clear();
        self.last_face_at = None;
        self.last_frame_at = None;
        self.typing = None;
    }

    fn enter_watching(&mut self, now: f64) {
        self.state = State::Watching;
        self.watching_since = now;
        self.frames.clear();
        self.last_face_at = None;
        self.last_frame_at = None;
    }

    fn alarm(&mut self, now: f64, reason: AlarmReason, mut events: Vec<Event>) -> Vec<Event> {
        self.state = State::Alarm { reason, since: now };
        self.typing = None;
        events.push(Event::AlarmStarted(reason));
        events
    }

    /// Has `pred` held continuously for at least `window` seconds up to `now`?
    /// Mismatching frames (or no face) shorter than `glitch_tolerance` don't
    /// break the streak, but the streak still needs the face in most frames
    /// and `agreement` of those frames matching.
    fn sustained(&self, now: f64, window: f64, pred: impl Fn(&Config, &Face) -> bool) -> bool {
        let hit = |frame: &Frame| matches!(frame, Frame::Face(f) if pred(&self.cfg, f));

        // Walk back from the newest frame to where the streak began. Trailing
        // misses are tolerated (a glitch) but don't count toward its length.
        let (mut start, mut end) = (None, None);
        let mut miss_since = None;
        for (t, frame) in self.frames.iter().rev() {
            if hit(frame) {
                start = Some(*t);
                end.get_or_insert(*t);
                miss_since = None;
            } else if *miss_since.get_or_insert(*t) - t > self.cfg.glitch_tolerance {
                break;
            }
        }
        let (Some(start), Some(end)) = (start, end) else {
            return false;
        };
        // Must still be going: a stalled camera can't keep an old streak alive.
        // (One frame interval of slack on top of the glitch tolerance.)
        if now - end > self.cfg.glitch_tolerance + 0.15 || end - start < window - 1e-9 {
            return false;
        }

        let (mut total, mut faces, mut hits) = (0u32, 0u32, 0u32);
        for (_, frame) in self
            .frames
            .iter()
            .filter(|(t, _)| (start..=end).contains(t))
        {
            total += 1;
            faces += u32::from(matches!(frame, Frame::Face(_)));
            hits += u32::from(hit(frame));
        }
        faces * 2 >= total && hits as f32 >= faces as f32 * self.cfg.agreement
    }

    /// Did the face look like it was nodding off just before it disappeared?
    fn was_drooping(&self, lost_at: f64) -> bool {
        let start = lost_at - self.cfg.droop_lookback;
        let (mut faces, mut drooping) = (0u32, 0u32);
        for (_, frame) in self
            .frames
            .iter()
            .filter(|(t, _)| *t >= start && *t <= lost_at)
        {
            if let Frame::Face(f) = frame {
                faces += 1;
                let droop = self.cfg.is_eye_closed(f.ear)
                    || self.cfg.is_head_down(f.pitch)
                    || f.roll.abs() > self.cfg.droop_roll;
                drooping += u32::from(droop);
            }
        }
        faces >= 3 && drooping * 2 >= faces
    }
}
