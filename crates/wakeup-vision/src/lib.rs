//! Face analysis for wakeup: camera frame in, eye openness and head pitch out.
//! Runs MediaPipe's face detector and face mesh models through ONNX Runtime.

#[cfg(feature = "camera")]
pub mod camera;
pub mod detector;
pub mod mesh;
pub mod metrics;
pub mod pipeline;
pub mod roi;

pub use metrics::FaceSample;
pub use pipeline::{FacePipeline, FrameResult};

use std::sync::Mutex;

use anyhow::{Result, anyhow};
use ort::environment::Environment;
use ort::session::{Session, builder::GraphOptimizationLevel};

/// ONNX Runtime allows a single environment per process, but pipelines get
/// rebuilt (after pauses, in tests), so create it once and share it.
fn environment() -> Result<Environment> {
    static ENV: Mutex<Option<Environment>> = Mutex::new(None);
    let mut env = ENV
        .lock()
        .map_err(|_| anyhow!("ONNX Runtime environment lock poisoned"))?;
    if let Some(env) = env.as_ref() {
        return Ok(env.clone());
    }
    let created = ort::init().with_name("wakeup").build()?;
    *env = Some(created.clone());
    Ok(created)
}

/// One intra-op thread: these models are tiny, and the app must stay light.
fn session(env: &Environment, model: &[u8]) -> Result<Session> {
    // Builder errors carry the builder back; drop it so anyhow can take them.
    let plain = ort::Error::<()>::from;
    let mut builder = Session::builder(env)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(plain)?
        .with_intra_threads(1)
        .map_err(plain)?;
    Ok(builder.commit_from_memory(model)?)
}
