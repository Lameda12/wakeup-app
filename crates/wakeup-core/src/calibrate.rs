//! Turns a short guided recording (eyes open, eyes closed, looking down)
//! into per-person thresholds.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationSample {
    pub ear: f32,
    pub pitch: f32,
}

/// Face samples per phase. Frames without a face are simply not added.
#[derive(Debug, Clone, Default)]
pub struct CalibrationData {
    pub open: Vec<CalibrationSample>,
    pub closed: Vec<CalibrationSample>,
    pub down: Vec<CalibrationSample>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CalibrationResult {
    /// Threshold to store as `Config::ear_closed`.
    pub ear_closed: f32,
    /// Typical open and closed readings, for showing the user the margin.
    pub ear_open_typical: f32,
    pub ear_shut_typical: f32,
    pub pitch_neutral: f32,
    /// `None` when looking down didn't move the reading enough to rely on;
    /// the head-down rule stays off and the face-lost rule covers it.
    pub pitch_down: Option<f32>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationError {
    TooFewFrames { phase: &'static str, got: usize },
    EyesOverlap { open_low: f32, closed_high: f32 },
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFrames { phase: "open", .. } => {
                write!(
                    f,
                    "the camera couldn't see your face. sit in front of it with some light on your face and try again."
                )
            }
            Self::TooFewFrames { phase, .. } => {
                write!(
                    f,
                    "lost your face while your eyes were {phase}. stay in view and try again."
                )
            }
            Self::EyesOverlap { .. } => write!(
                f,
                "open and closed eyes looked too similar. face the screen, close your eyes fully, and try again. glasses with glare can cause this."
            ),
        }
    }
}

/// Fewer than this many face frames in a phase (1.5s at 10 fps) can't be trusted.
pub const MIN_FRAMES: usize = 15;
/// Looking down must move pitch at least this far from neutral to be usable.
pub const MIN_PITCH_SWING: f32 = 10.0;

pub fn percentile(values: &[f32], q: f32) -> f32 {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q.clamp(0.0, 1.0)).round() as usize]
}

pub fn calibrate(data: &CalibrationData) -> Result<CalibrationResult, CalibrationError> {
    for (phase, samples) in [("open", &data.open), ("closed", &data.closed)] {
        if samples.len() < MIN_FRAMES {
            return Err(CalibrationError::TooFewFrames {
                phase,
                got: samples.len(),
            });
        }
    }
    let ears = |s: &[CalibrationSample]| s.iter().map(|x| x.ear).collect::<Vec<_>>();
    let pitches = |s: &[CalibrationSample]| s.iter().map(|x| x.pitch).collect::<Vec<_>>();
    let (open, closed) = (ears(&data.open), ears(&data.closed));

    // Nearly all open frames must read above nearly all closed frames.
    let (open_low, closed_high) = (percentile(&open, 0.1), percentile(&closed, 0.9));
    if open_low <= closed_high {
        return Err(CalibrationError::EyesOverlap {
            open_low,
            closed_high,
        });
    }
    let (ear_open_typical, ear_shut_typical) = (percentile(&open, 0.5), percentile(&closed, 0.5));
    // Midway between the overlap edges: the widest margin both ways.
    let ear_closed = (open_low + closed_high) / 2.0;

    let pitch_neutral = percentile(&pitches(&data.open), 0.5);
    let mut warnings = vec![];
    let pitch_down = if data.down.len() < MIN_FRAMES {
        warnings.push(
            "your face left the camera when you looked down, so wakeup will treat a disappearing, drooping face as dozing instead.".into(),
        );
        None
    } else {
        let down = percentile(&pitches(&data.down), 0.5);
        if (down - pitch_neutral).abs() < MIN_PITCH_SWING {
            warnings.push("looking down barely changed your head angle on camera, so the head-down alarm is off. recalibrate with a bigger nod to turn it on.".into());
            None
        } else {
            Some(down)
        }
    };
    if ear_open_typical - ear_shut_typical < 0.08 {
        warnings.push("the gap between open and closed eyes is small. if you get false alarms, recalibrate in better light.".into());
    }

    Ok(CalibrationResult {
        ear_closed,
        ear_open_typical,
        ear_shut_typical,
        pitch_neutral,
        pitch_down,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(n: usize, ear: f32, pitch: f32) -> Vec<CalibrationSample> {
        // A little deterministic jitter so percentiles mean something.
        (0..n)
            .map(|i| CalibrationSample {
                ear: ear + ((i % 5) as f32 - 2.0) * 0.005,
                pitch: pitch + ((i % 3) as f32 - 1.0),
            })
            .collect()
    }

    #[test]
    fn clean_recording_calibrates() {
        let data = CalibrationData {
            open: samples(40, 0.30, 8.0),
            closed: samples(30, 0.08, 9.0),
            down: samples(30, 0.22, 34.0),
        };
        let r = calibrate(&data).unwrap();
        assert!((0.15..0.24).contains(&r.ear_closed), "{r:?}");
        assert_eq!(r.pitch_neutral, 8.0);
        assert_eq!(r.pitch_down, Some(34.0));
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }

    #[test]
    fn negative_pitch_cameras_work_too() {
        let data = CalibrationData {
            open: samples(40, 0.30, 5.0),
            closed: samples(30, 0.08, 5.0),
            down: samples(30, 0.22, -22.0),
        };
        assert_eq!(calibrate(&data).unwrap().pitch_down, Some(-22.0));
    }

    #[test]
    fn no_face_is_a_clear_error() {
        let data = CalibrationData {
            open: samples(3, 0.3, 0.0),
            ..Default::default()
        };
        let err = calibrate(&data).unwrap_err();
        assert_eq!(
            err,
            CalibrationError::TooFewFrames {
                phase: "open",
                got: 3
            }
        );
        assert!(err.to_string().contains("couldn't see your face"));
    }

    #[test]
    fn eyes_that_never_closed_fail() {
        let data = CalibrationData {
            open: samples(40, 0.30, 8.0),
            closed: samples(30, 0.29, 8.0),
            down: samples(30, 0.25, 30.0),
        };
        assert!(matches!(
            calibrate(&data),
            Err(CalibrationError::EyesOverlap { .. })
        ));
    }

    #[test]
    fn small_nod_disables_head_down_with_warning() {
        let data = CalibrationData {
            open: samples(40, 0.30, 8.0),
            closed: samples(30, 0.08, 8.0),
            down: samples(30, 0.28, 12.0),
        };
        let r = calibrate(&data).unwrap();
        assert_eq!(r.pitch_down, None);
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn face_leaving_when_looking_down_is_fine() {
        let data = CalibrationData {
            open: samples(40, 0.30, 8.0),
            closed: samples(30, 0.08, 8.0),
            down: samples(4, 0.2, 40.0),
        };
        let r = calibrate(&data).unwrap();
        assert_eq!(r.pitch_down, None);
        assert!(r.warnings[0].contains("left the camera"));
    }

    #[test]
    fn narrow_margin_warns() {
        let data = CalibrationData {
            open: samples(40, 0.24, 8.0),
            closed: samples(30, 0.18, 8.0),
            down: samples(30, 0.2, 35.0),
        };
        let r = calibrate(&data).unwrap();
        assert!(
            r.warnings.iter().any(|w| w.contains("small")),
            "{:?}",
            r.warnings
        );
    }
}
