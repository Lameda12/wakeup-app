//! A calibration run in progress: the UI walks the user through phases,
//! the monitor loop files face samples under the current one.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use wakeup_core::{CalibrationData, CalibrationSample};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Open,
    Closed,
    Down,
}

/// Ignore the first second of each phase while the person is still moving.
const SETTLE_SECS: f64 = 1.0;

#[derive(Debug, Default)]
pub struct Session {
    phase: Option<(Phase, Instant)>,
    pub data: CalibrationData,
}

impl Session {
    /// Starts (or restarts) a phase, discarding anything recorded for it.
    pub fn set_phase(&mut self, phase: Phase) {
        self.bucket(phase).clear();
        self.phase = Some((phase, Instant::now()));
    }

    pub fn record(&mut self, ear: f32, pitch: f32) {
        let Some((phase, since)) = self.phase else {
            return;
        };
        if since.elapsed().as_secs_f64() >= SETTLE_SECS {
            self.bucket(phase).push(CalibrationSample { ear, pitch });
        }
    }

    fn bucket(&mut self, phase: Phase) -> &mut Vec<CalibrationSample> {
        match phase {
            Phase::Open => &mut self.data.open,
            Phase::Closed => &mut self.data.closed,
            Phase::Down => &mut self.data.down,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_only_into_current_phase_after_settling() {
        let mut s = Session::default();
        s.record(0.3, 0.0);
        assert!(s.data.open.is_empty(), "no phase yet");
        s.set_phase(Phase::Open);
        s.record(0.3, 0.0);
        assert!(s.data.open.is_empty(), "still settling");
        s.phase = Some((
            Phase::Open,
            Instant::now() - std::time::Duration::from_secs(2),
        ));
        s.record(0.3, 0.0);
        assert_eq!(s.data.open.len(), 1);
        s.set_phase(Phase::Open);
        assert!(s.data.open.is_empty(), "redoing a phase starts it fresh");
    }
}
