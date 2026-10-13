//! Everything the user can change. Stored as one flat `config.json`, so
//! files from M1 (bare engine config) still load.

use serde::{Deserialize, Serialize};
use wakeup_core::{ActiveHours, Config};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(flatten)]
    pub engine: Config,
    /// Thresholds came from a calibration rather than defaults.
    pub calibrated: bool,
    /// First-run setup finished; otherwise it opens on launch.
    pub onboarded: bool,
    pub launch_at_login: bool,
    /// Pause while any app is using the microphone (calls).
    pub meeting_pause: bool,
    pub active_hours_enabled: bool,
    pub active_hours: ActiveHours,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            engine: Config::default(),
            calibrated: false,
            onboarded: false,
            launch_at_login: true,
            meeting_pause: true,
            active_hours_enabled: false,
            active_hours: ActiveHours::default(),
        }
    }
}

fn check(ok: bool, msg: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(msg.to_string()) }
}

impl Settings {
    /// Rejects values that would make the app useless or dangerous to rely on.
    pub fn validate(&self) -> Result<(), String> {
        let e = &self.engine;
        check(
            (1.0..=10.0).contains(&e.eyes_closed_for),
            "eyes-closed time must be 1-10 seconds",
        )?;
        check(
            (1.0..=10.0).contains(&e.head_down_for),
            "head-down time must be 1-10 seconds",
        )?;
        check(
            e.idle_nudge_after == 0.0 || (30.0..=1800.0).contains(&e.idle_nudge_after),
            "the nudge must come after 30 seconds to 30 minutes, or be off",
        )?;
        check(
            (5.0..=60.0).contains(&e.nudge_timeout),
            "the nudge must wait 5-60 seconds",
        )?;
        check(
            (0.0..=60.0).contains(&e.camera_gate),
            "camera wait must be 0-60 seconds",
        )?;
        check(
            (0.03..=0.45).contains(&e.ear_closed),
            "eye threshold is out of range; recalibrate",
        )?;
        check(
            !self.active_hours_enabled
                || (self.active_hours.is_valid() && !self.active_hours.days.is_empty()),
            "active hours need a valid start, end and at least one day",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m1_flat_config_still_loads() {
        let s: Settings =
            serde_json::from_str(r#"{ "ear_closed": 0.17, "pitch_down": -20.0 }"#).unwrap();
        assert_eq!(s.engine.ear_closed, 0.17);
        assert_eq!(s.engine.pitch_down, -20.0);
        assert!(!s.onboarded);
        assert!(s.meeting_pause);
    }

    #[test]
    fn round_trips_flat() {
        let s = Settings {
            onboarded: true,
            ..Settings::default()
        };
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("engine").is_none(), "engine fields are flattened");
        assert_eq!(json["eyes_closed_for"], 2.5);
        assert_eq!(serde_json::from_value::<Settings>(json).unwrap(), s);
    }

    #[test]
    fn validation() {
        assert!(Settings::default().validate().is_ok());
        let mut s = Settings::default();
        s.engine.idle_nudge_after = 0.0;
        assert!(s.validate().is_ok(), "idle rule off is allowed");
        s.engine.idle_nudge_after = 5.0;
        assert!(s.validate().is_err());
        let mut s = Settings {
            active_hours_enabled: true,
            ..Settings::default()
        };
        s.active_hours.days.clear();
        assert!(s.validate().is_err());
    }
}
