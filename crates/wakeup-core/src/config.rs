/// Every threshold the engine uses. Times are in seconds.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(default)
)]
pub struct Config {
    /// Input idle time before the camera turns on. 0 = camera always on.
    pub camera_gate: f64,

    /// Eye aspect ratio below which an eye counts as closed (from calibration).
    pub ear_closed: f32,
    /// Eyes closed this long fires the alarm.
    pub eyes_closed_for: f64,

    /// Calibrated head pitch looking at the screen, and looking down at a phone.
    /// The sign differs between cameras, so the rule works off both points.
    pub pitch_neutral: f32,
    pub pitch_down: f32,
    /// Fraction of the way from neutral to down that counts as "down".
    pub pitch_down_fraction: f32,
    /// Head down this long fires the alarm.
    pub head_down_for: f64,

    /// A run of mismatching frames shorter than this doesn't break a streak
    /// (a misread frame, a flicker of the eyelids).
    pub glitch_tolerance: f64,
    /// Fraction of face frames within a streak that must match.
    pub agreement: f32,

    /// No input this long (face present) shows the "still here?" nudge.
    pub idle_nudge_after: f64,
    /// Nudge ignored this long fires the alarm.
    pub nudge_timeout: f64,

    /// No face this long means you walked away: no alarms.
    pub away_after: f64,
    /// ...unless the face vanished while drooping (head on the desk). Then the
    /// alarm fires after this much shorter wait.
    pub lost_while_drowsy_after: f64,
    /// How far back before the face vanished to look for drooping.
    pub droop_lookback: f64,
    /// Head roll beyond this (degrees, either side) counts as drooping.
    pub droop_roll: f32,

    /// Dismissing needs eyes open in at least this fraction of face samples
    /// while the phrase is typed.
    pub dismiss_eyes_open: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            camera_gate: 8.0,
            ear_closed: 0.20,
            eyes_closed_for: 2.5,
            pitch_neutral: 10.0,
            pitch_down: 35.0,
            pitch_down_fraction: 0.6,
            head_down_for: 3.0,
            glitch_tolerance: 0.3,
            agreement: 0.8,
            idle_nudge_after: 90.0,
            nudge_timeout: 10.0,
            away_after: 10.0,
            lost_while_drowsy_after: 3.0,
            droop_lookback: 3.0,
            droop_roll: 35.0,
            dismiss_eyes_open: 0.9,
        }
    }
}

impl Config {
    pub fn is_eye_closed(&self, ear: f32) -> bool {
        ear < self.ear_closed
    }

    /// True once pitch has moved `pitch_down_fraction` of the way from neutral
    /// toward the calibrated "down" pose, whichever direction that is.
    pub fn is_head_down(&self, pitch: f32) -> bool {
        let span = self.pitch_down - self.pitch_neutral;
        if span.abs() < f32::EPSILON {
            return false;
        }
        (pitch - self.pitch_neutral) / span >= self.pitch_down_fraction
    }

    /// Longest window any rule looks back over.
    pub(crate) fn history(&self) -> f64 {
        self.eyes_closed_for
            .max(self.head_down_for)
            .max(self.droop_lookback)
            + 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_down_works_for_either_pitch_sign() {
        let positive = Config {
            pitch_neutral: 10.0,
            pitch_down: 35.0,
            ..Config::default()
        };
        assert!(!positive.is_head_down(15.0));
        assert!(positive.is_head_down(30.0));
        assert!(!positive.is_head_down(-20.0), "looking up is not down");

        let negative = Config {
            pitch_neutral: 5.0,
            pitch_down: -20.0,
            ..Config::default()
        };
        assert!(negative.is_head_down(-15.0));
        assert!(!negative.is_head_down(20.0));

        let uncalibrated = Config {
            pitch_neutral: 5.0,
            pitch_down: 5.0,
            ..Config::default()
        };
        assert!(!uncalibrated.is_head_down(90.0));
    }
}
