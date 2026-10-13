use std::time::{Duration, Instant};

use anyhow::Result;
use image::RgbImage;

use crate::detector::{Detection, Detector};
use crate::mesh::{Mesh, MeshModel};
use crate::metrics::{EYE_LINE, FaceSample};
use crate::roi::roi_around;

/// Below this, the mesh model thinks it lost the face and we re-detect.
const MIN_PRESENCE: f32 = 0.5;
/// MediaPipe's crop margin around a face for the landmark model.
const ROI_SCALE: f32 = 1.5;

#[derive(Debug, Clone, Default)]
pub struct Timings {
    pub detect: Option<Duration>,
    pub mesh: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct FrameResult {
    pub sample: Option<FaceSample>,
    pub mesh: Option<Mesh>,
    pub detection: Option<Detection>,
    pub timings: Timings,
}

/// Detector + landmark model with frame-to-frame tracking: the detector only
/// runs when there's no face from the previous frame to follow.
pub struct FacePipeline {
    detector: Detector,
    mesh: MeshModel,
    tracked: Option<Mesh>,
}

impl FacePipeline {
    pub fn new() -> Result<Self> {
        let env = crate::environment()?;
        Ok(Self {
            detector: Detector::new(&env)?,
            mesh: MeshModel::new(&env)?,
            tracked: None,
        })
    }

    /// Forget the tracked face, e.g. after the camera was paused.
    pub fn reset(&mut self) {
        self.tracked = None;
    }

    pub fn process(&mut self, img: &RgbImage) -> Result<FrameResult> {
        let mut timings = Timings::default();
        let mut detection = None;

        let roi = match self.tracked.take() {
            Some(prev) => {
                let pts: Vec<_> = prev.points.iter().map(|p| (p[0], p[1])).collect();
                roi_around(&pts, prev.xy(EYE_LINE[0]), prev.xy(EYE_LINE[1]), ROI_SCALE)
            }
            None => {
                let t = Instant::now();
                let found = self.detector.detect(img)?;
                timings.detect = Some(t.elapsed());
                let Some(d) = found else {
                    return Ok(FrameResult {
                        sample: None,
                        mesh: None,
                        detection: None,
                        timings,
                    });
                };
                let (x, y, w, h) = (d.bbox[0], d.bbox[1], d.bbox[2], d.bbox[3]);
                let corners = [(x - w / 2.0, y - h / 2.0), (x + w / 2.0, y + h / 2.0)];
                let roi = roi_around(&corners, d.keypoints[0], d.keypoints[1], ROI_SCALE);
                detection = Some(d);
                roi
            }
        };

        let t = Instant::now();
        let mesh = self.mesh.run(img, roi)?;
        timings.mesh = Some(t.elapsed());

        if mesh.presence < MIN_PRESENCE {
            return Ok(FrameResult {
                sample: None,
                mesh: Some(mesh),
                detection,
                timings,
            });
        }
        self.tracked = Some(mesh.clone());
        Ok(FrameResult {
            sample: Some(FaceSample::from(&mesh)),
            mesh: Some(mesh),
            detection,
            timings,
        })
    }
}
