use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use wakeup_vision::FrameResult;

/// One analyzed frame, flattened for printing and CSV.
#[derive(Debug, Clone)]
pub struct Row {
    pub t: f64,
    pub phase: &'static str,
    pub tag: String,
    pub face: bool,
    pub presence: f32,
    pub ear_l: f32,
    pub ear_r: f32,
    pub pitch: f32,
    pub roll: f32,
    pub idle: Option<f64>,
    pub detect_ms: Option<f64>,
    pub mesh_ms: Option<f64>,
}

impl Row {
    pub fn new(t: Duration, r: &FrameResult, idle: Option<f64>) -> Self {
        let s = r.sample;
        Self {
            t: t.as_secs_f64(),
            phase: "",
            tag: String::new(),
            face: s.is_some(),
            presence: r.mesh.as_ref().map_or(0.0, |m| m.presence),
            ear_l: s.map_or(f32::NAN, |s| s.ear_left),
            ear_r: s.map_or(f32::NAN, |s| s.ear_right),
            pitch: s.map_or(f32::NAN, |s| s.pitch_deg),
            roll: s.map_or(f32::NAN, |s| s.roll_deg),
            idle,
            detect_ms: r.timings.detect.map(|d| d.as_secs_f64() * 1e3),
            mesh_ms: r.timings.mesh.map(|d| d.as_secs_f64() * 1e3),
        }
    }

    pub fn ear(&self) -> f32 {
        (self.ear_l + self.ear_r) / 2.0
    }

    pub fn infer_ms(&self) -> f64 {
        self.detect_ms.unwrap_or(0.0) + self.mesh_ms.unwrap_or(0.0)
    }

    pub fn pretty(&self) -> String {
        let idle = self.idle.map_or("n/a".into(), |s| format!("{s:.1}s"));
        if !self.face {
            return format!(
                "{:6.1}s  no face                                     idle {idle:>6}",
                self.t
            );
        }
        // A crude bar so eye closure is visible at a glance.
        let bar = "#".repeat((self.ear().clamp(0.0, 0.4) * 50.0) as usize);
        format!(
            "{:6.1}s  ear {:.3} {bar:<20}  pitch {:+6.1}°  roll {:+6.1}°  idle {idle:>6}  {:.1}ms",
            self.t,
            self.ear(),
            self.pitch,
            self.roll,
            self.infer_ms()
        )
    }
}

pub struct Recorder(BufWriter<File>);

impl Recorder {
    pub fn create(path: &Path) -> Result<Self> {
        let mut w = BufWriter::new(
            File::create(path).with_context(|| format!("creating {}", path.display()))?,
        );
        writeln!(
            w,
            "t,tag,phase,face,presence,ear_l,ear_r,ear,pitch,roll,idle,detect_ms,mesh_ms"
        )?;
        Ok(Self(w))
    }

    pub fn write(&mut self, r: &Row) -> Result<()> {
        let opt = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.3}"));
        let num = |v: f32| {
            if v.is_nan() {
                String::new()
            } else {
                format!("{v:.4}")
            }
        };
        writeln!(
            self.0,
            "{:.3},{},{},{},{:.3},{},{},{},{},{},{},{},{}",
            r.t,
            r.tag,
            r.phase,
            r.face as u8,
            r.presence,
            num(r.ear_l),
            num(r.ear_r),
            num(r.ear()),
            num(r.pitch),
            num(r.roll),
            opt(r.idle),
            opt(r.detect_ms),
            opt(r.mesh_ms)
        )?;
        self.0.flush()?;
        Ok(())
    }
}
