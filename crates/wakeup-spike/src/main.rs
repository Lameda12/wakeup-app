//! M0 spike: does webcam eye/head tracking separate cleanly on real hardware?

mod annotate;
mod record;
mod session;
mod stats;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wakeup_vision::FacePipeline;

use crate::record::{Recorder, Row};

#[derive(Parser)]
#[command(name = "wakeup-spike", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Guided ~50s recording: eyes open, closed, head down, blinking, away. Prints a verdict.
    Session {
        /// Label for this run, e.g. "glasses" or "low-light".
        #[arg(long, default_value = "default")]
        tag: String,
        #[arg(long, default_value = "session.csv")]
        out: PathBuf,
        #[command(flatten)]
        cam: CamArgs,
    },
    /// Print live metrics from the webcam until Ctrl+C.
    Live {
        #[arg(long)]
        csv: Option<PathBuf>,
        #[command(flatten)]
        cam: CamArgs,
    },
    /// Analyze a still image; optionally write a copy with landmarks drawn on it.
    Image {
        path: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Print keyboard/trackpad idle time once a second until Ctrl+C.
    Idle,
}

#[derive(clap::Args, Clone, Copy)]
pub struct CamArgs {
    /// Camera index (0 = built-in on most Macs).
    #[arg(long, default_value_t = 0)]
    camera: u32,
    /// Frames analyzed per second.
    #[arg(long, default_value_t = 10)]
    fps: u32,
}

fn main() -> Result<()> {
    // SAFETY: first statement of main; no other threads exist yet.
    unsafe { wakeup_vision::disable_onnx_telemetry() };
    match Cli::parse().cmd {
        Cmd::Session { tag, out, cam } => session::run(&tag, &out, cam),
        Cmd::Live { csv, cam } => live(csv, cam),
        Cmd::Image { path, out } => image_cmd(&path, out),
        Cmd::Idle => idle(),
    }
}

fn image_cmd(path: &PathBuf, out: Option<PathBuf>) -> Result<()> {
    let img = image::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .to_rgb8();
    let mut pipeline = FacePipeline::new()?;
    // Second pass crops from the first pass's landmarks, like live tracking does.
    pipeline.process(&img)?;
    let result = pipeline.process(&img)?;
    match &result.sample {
        Some(s) => println!(
            "face {:.2}  ear L {:.3} R {:.3} avg {:.3}  pitch {:+.1}°  roll {:+.1}°",
            s.presence,
            s.ear_left,
            s.ear_right,
            s.ear(),
            s.pitch_deg,
            s.roll_deg
        ),
        None => println!("no face"),
    }
    if let Some(out) = out {
        annotate::draw(&img, &result)
            .save(&out)
            .with_context(|| format!("writing {}", out.display()))?;
        println!("wrote {}", out.display());
    }
    Ok(())
}

fn live(csv: Option<PathBuf>, cam: CamArgs) -> Result<()> {
    let mut recorder = csv.as_deref().map(Recorder::create).transpose()?;
    run_camera(cam, |row| {
        println!("{}", row.pretty());
        if let Some(r) = recorder.as_mut() {
            r.write(&row)?;
        }
        Ok(true)
    })
}

fn idle() -> Result<()> {
    loop {
        match wakeup_platform::idle_seconds() {
            Some(s) => println!("idle {s:.1}s"),
            None => println!("idle time not supported on this OS yet"),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Opens the camera and calls `on_row` for each analyzed frame at `cam.fps`
/// until it returns `Ok(false)`.
#[cfg(feature = "camera")]
pub fn run_camera(cam: CamArgs, mut on_row: impl FnMut(Row) -> Result<bool>) -> Result<()> {
    use wakeup_vision::camera::{Camera, request_permission};

    request_permission()?;
    let mut camera = Camera::open(cam.camera, 640, 480)?;
    let (w, h) = camera.resolution();
    eprintln!(
        "camera {} at {w}x{h}, analyzing {} fps",
        cam.camera, cam.fps
    );

    let mut pipeline = FacePipeline::new()?;
    let period = Duration::from_secs_f64(1.0 / cam.fps.max(1) as f64);
    let start = Instant::now();
    let mut next = start;
    loop {
        let frame = camera.frame()?;
        let result = pipeline.process(&frame)?;
        let row = Row::new(start.elapsed(), &result, wakeup_platform::idle_seconds());
        if !on_row(row)? {
            return Ok(());
        }
        next += period;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        } else {
            next = Instant::now();
        }
    }
}

#[cfg(not(feature = "camera"))]
pub fn run_camera(_: CamArgs, _: impl FnMut(Row) -> Result<bool>) -> Result<()> {
    anyhow::bail!("built without the `camera` feature")
}

/// Process CPU time (user + system) so far.
pub fn cpu_time() -> Duration {
    // SAFETY: getrusage only writes into the zeroed struct we pass it.
    let usage = unsafe {
        let mut u: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut u);
        u
    };
    let tv = |t: libc::timeval| {
        Duration::from_secs(t.tv_sec as u64) + Duration::from_micros(t.tv_usec as u64)
    };
    tv(usage.ru_utime) + tv(usage.ru_stime)
}
