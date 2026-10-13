//! MediaPipe face mesh: 468 3D landmarks from a face crop.

use anyhow::{Context, Result};
use image::RgbImage;
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

use crate::roi::Roi;
use crate::session;

const MODEL: &[u8] = include_bytes!("../../../models/face_landmark.onnx");
const INPUT: u32 = 192;
pub const NUM_LANDMARKS: usize = 468;

/// Landmarks in image pixels. `z` is depth relative to the face center, in
/// roughly the same pixel scale as `x` (smaller = closer to the camera).
#[derive(Debug, Clone)]
pub struct Mesh {
    pub points: Vec<[f32; 3]>,
    /// Model's confidence that the crop actually contains a face.
    pub presence: f32,
    pub roi: Roi,
}

pub struct MeshModel {
    session: Session,
}

impl MeshModel {
    pub fn new(env: &Environment) -> Result<Self> {
        Ok(Self {
            session: session(env, MODEL).context("loading face mesh")?,
        })
    }

    pub fn run(&mut self, img: &RgbImage, roi: Roi) -> Result<Mesh> {
        let input = Tensor::from_array(([1usize, 192, 192, 3], roi.sample(img, INPUT, 0.0, 1.0)))?;
        let outputs = self.session.run(ort::inputs!["input_1" => input])?;
        let (_, raw) = outputs["conv2d_21"].try_extract_tensor::<f32>()?;
        let (_, flag) = outputs["conv2d_31"].try_extract_tensor::<f32>()?;

        let n = INPUT as f32;
        let z_scale = roi.size / n;
        let points = raw
            .as_chunks::<3>()
            .0
            .iter()
            .take(NUM_LANDMARKS)
            .map(|p| {
                let (x, y) = roi.to_image(p[0] / n, p[1] / n);
                [x, y, p[2] * z_scale]
            })
            .collect();
        Ok(Mesh {
            points,
            presence: 1.0 / (1.0 + (-flag[0]).exp()),
            roi,
        })
    }
}

impl Mesh {
    pub fn xy(&self, i: usize) -> (f32, f32) {
        (self.points[i][0], self.points[i][1])
    }

    /// Axis-aligned bounding box: min x, min y, max x, max y.
    pub fn bounds(&self) -> [f32; 4] {
        self.points
            .iter()
            .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            })
    }
}
