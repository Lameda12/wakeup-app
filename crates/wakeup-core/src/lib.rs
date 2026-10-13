//! The wakeup state machine. Feed it input idle time and camera frames on a
//! clock; it says what the camera should do and when to nudge or alarm.
//! Pure: no camera, no clock, no OS calls, so every rule is unit-tested.

mod calibrate;
mod config;
mod engine;
mod phrase;
mod schedule;

pub use calibrate::{
    CalibrationData, CalibrationError, CalibrationResult, CalibrationSample, calibrate, percentile,
};
pub use config::Config;
pub use engine::{
    AlarmReason, Camera, DismissError, Engine, Event, Face, Frame, Input, PauseError, PauseReason,
    State,
};
pub use phrase::{matches_phrase, pick_phrase};
pub use schedule::{ActiveHours, parse_hhmm};

#[cfg(test)]
mod tests;
