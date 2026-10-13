//! MediaPipe BlazeFace (short range) face detector.

use anyhow::{Context, Result};
use image::RgbImage;
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

use crate::roi::Roi;
use crate::session;

const MODEL: &[u8] = include_bytes!("../../../models/face_detection_short_range.onnx");
const INPUT: u32 = 128;
const NUM_ANCHORS: usize = 896;
const NUM_COORDS: usize = 16;
const MIN_SCORE: f32 = 0.5;
const NMS_IOU: f32 = 0.3;

/// A detected face in image pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub score: f32,
    /// Center x, center y, width, height.
    pub bbox: [f32; 4],
    /// Subject's right eye, left eye, nose tip, mouth, right ear, left ear.
    pub keypoints: [(f32, f32); 6],
}

pub struct Detector {
    session: Session,
    anchors: Vec<(f32, f32)>,
}

impl Detector {
    pub fn new(env: &Environment) -> Result<Self> {
        Ok(Self {
            session: session(env, MODEL).context("loading face detector")?,
            anchors: anchors(),
        })
    }

    /// Returns the most confident face, if any.
    pub fn detect(&mut self, img: &RgbImage) -> Result<Option<Detection>> {
        let roi = Roi::letterbox(img.width(), img.height());
        let input = Tensor::from_array(([1usize, 128, 128, 3], roi.sample(img, INPUT, -1.0, 1.0)))?;
        let outputs = self.session.run(ort::inputs!["input" => input])?;
        let (_, boxes) = outputs["regressors"].try_extract_tensor::<f32>()?;
        let (_, scores) = outputs["classificators"].try_extract_tensor::<f32>()?;

        let candidates = decode(boxes, scores, &self.anchors);
        Ok(best_weighted(candidates).map(|d| d.to_image(&roi)))
    }
}

/// SSD anchor centers for the short-range model: a 16x16 grid with 2 anchors
/// per cell (stride 8) followed by an 8x8 grid with 6 per cell (stride 16).
fn anchors() -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(NUM_ANCHORS);
    for (grid, per_cell) in [(16, 2), (8, 6)] {
        for y in 0..grid {
            for x in 0..grid {
                let center = (
                    (x as f32 + 0.5) / grid as f32,
                    (y as f32 + 0.5) / grid as f32,
                );
                out.extend(std::iter::repeat_n(center, per_cell));
            }
        }
    }
    out
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-100.0, 100.0)).exp())
}

/// Decodes raw outputs into detections in normalized [0,1] letterbox space.
fn decode(boxes: &[f32], scores: &[f32], anchors: &[(f32, f32)]) -> Vec<Detection> {
    let scale = INPUT as f32;
    anchors
        .iter()
        .enumerate()
        .filter_map(|(i, &(ax, ay))| {
            let score = sigmoid(scores[i]);
            if score < MIN_SCORE {
                return None;
            }
            let r = &boxes[i * NUM_COORDS..(i + 1) * NUM_COORDS];
            let mut keypoints = [(0.0, 0.0); 6];
            for (k, kp) in keypoints.iter_mut().enumerate() {
                *kp = (r[4 + 2 * k] / scale + ax, r[5 + 2 * k] / scale + ay);
            }
            Some(Detection {
                score,
                bbox: [
                    r[0] / scale + ax,
                    r[1] / scale + ay,
                    r[2] / scale,
                    r[3] / scale,
                ],
                keypoints,
            })
        })
        .collect()
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let (ax0, ay0, ax1, ay1) = (
        a[0] - a[2] / 2.0,
        a[1] - a[3] / 2.0,
        a[0] + a[2] / 2.0,
        a[1] + a[3] / 2.0,
    );
    let (bx0, by0, bx1, by1) = (
        b[0] - b[2] / 2.0,
        b[1] - b[3] / 2.0,
        b[0] + b[2] / 2.0,
        b[1] + b[3] / 2.0,
    );
    let iw = (ax1.min(bx1) - ax0.max(bx0)).max(0.0);
    let ih = (ay1.min(by1) - ay0.max(by0)).max(0.0);
    let inter = iw * ih;
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union > 0.0 { inter / union } else { 0.0 }
}

/// MediaPipe-style weighted NMS, keeping only the top cluster: we track one face.
fn best_weighted(candidates: Vec<Detection>) -> Option<Detection> {
    let top = candidates
        .iter()
        .max_by(|a, b| a.score.total_cmp(&b.score))?
        .clone();
    let cluster: Vec<_> = candidates
        .iter()
        .filter(|d| iou(&d.bbox, &top.bbox) > NMS_IOU)
        .collect();
    let total: f32 = cluster.iter().map(|d| d.score).sum();
    let mut merged = Detection {
        score: top.score,
        bbox: [0.0; 4],
        keypoints: [(0.0, 0.0); 6],
    };
    for d in &cluster {
        let w = d.score / total;
        for (m, v) in merged.bbox.iter_mut().zip(d.bbox) {
            *m += v * w;
        }
        for (m, v) in merged.keypoints.iter_mut().zip(d.keypoints) {
            m.0 += v.0 * w;
            m.1 += v.1 * w;
        }
    }
    Some(merged)
}

impl Detection {
    fn to_image(&self, roi: &Roi) -> Detection {
        let (cx, cy) = roi.to_image(self.bbox[0], self.bbox[1]);
        Detection {
            score: self.score,
            bbox: [cx, cy, self.bbox[2] * roi.size, self.bbox[3] * roi.size],
            keypoints: self.keypoints.map(|(x, y)| roi.to_image(x, y)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_count_and_layout() {
        let a = anchors();
        assert_eq!(a.len(), NUM_ANCHORS);
        assert_eq!(a[0], (1.0 / 32.0, 1.0 / 32.0));
        assert_eq!(a[0], a[1]);
        assert_eq!(a[512], (1.0 / 16.0, 1.0 / 16.0));
    }

    #[test]
    fn weighted_nms_merges_overlaps_and_ignores_others() {
        let mk = |score, x| Detection {
            score,
            bbox: [x, 0.5, 0.2, 0.2],
            keypoints: [(x, 0.5); 6],
        };
        let best = best_weighted(vec![mk(0.9, 0.50), mk(0.9, 0.52), mk(0.8, 0.1)]).unwrap();
        assert!((best.bbox[0] - 0.51).abs() < 1e-4);
        assert_eq!(best.score, 0.9);
        assert!(best_weighted(vec![]).is_none());
    }
}
