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

/// Stops ONNX Runtime's built-in usage telemetry. Microsoft's official builds
/// include a client that posts to mobile.events.data.microsoft.com, and the
/// API-level switch (`with_telemetry(false)`) doesn't stop the connections;
/// only this variable does (measured: 3 attempts per 25s without it, 0 with).
/// Leaves an explicit user setting alone.
///
/// # Safety
/// Call at the very start of `main`, before any other thread exists:
/// changing the environment while another thread reads it is undefined
/// behavior.
pub unsafe fn disable_onnx_telemetry() {
    if std::env::var_os("ORT_DISABLE_TELEMETRY").is_none() {
        // SAFETY: the caller guarantees the process is still single-threaded.
        unsafe { std::env::set_var("ORT_DISABLE_TELEMETRY", "1") };
    }
}

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
    // Belt and braces with `disable_onnx_telemetry`: nothing leaves the machine.
    let created = ort::init()
        .with_name("wakeup")
        .with_telemetry(false)
        .build()?;
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
