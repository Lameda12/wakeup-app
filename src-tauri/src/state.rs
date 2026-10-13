use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Wry};
use wakeup_core::{AlarmReason, Engine};
use wakeup_platform::volume::VolumeGuard;

use crate::audio::Audio;
use crate::calibration;
use crate::settings::Settings;
use crate::store::{self, EventLog};

/// What the alarm windows show.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmView {
    pub reason: AlarmReason,
    pub phrase: String,
    pub started_at_ms: u64,
}

/// What the nudge window shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NudgeView {
    pub timeout_secs: f64,
    pub shown_at_ms: u64,
}

pub struct Shared {
    pub engine: Engine,
    pub settings: Settings,
    config_dir: PathBuf,
    started: Instant,
    pub alarm: Option<AlarmView>,
    pub nudge: Option<NudgeView>,
    pub volume: Option<VolumeGuard>,
    /// While set, the engine is frozen and the camera feeds calibration.
    pub calibration: Option<calibration::Session>,
}

impl Shared {
    pub fn new(settings: Settings, config_dir: PathBuf) -> Self {
        Self {
            engine: Engine::new(settings.engine.clone()),
            settings,
            config_dir,
            started: Instant::now(),
            alarm: None,
            nudge: None,
            volume: None,
            calibration: None,
        }
    }

    /// Engine clock: monotonic seconds since launch.
    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Writes settings to disk and points the engine at the new thresholds.
    pub fn apply_settings(&mut self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        store::save_settings(&self.config_dir, &settings)
            .map_err(|e| format!("couldn't save settings: {e}"))?;
        self.engine.set_config(settings.engine.clone());
        self.settings = settings;
        Ok(())
    }

    /// Fresh engine state (after calibration): nothing carried over from
    /// before the camera was taken over.
    pub fn restart_engine(&mut self) {
        self.engine = Engine::new(self.settings.engine.clone());
    }
}

/// Everything the monitor thread, commands and tray need. Cheap to clone.
#[derive(Clone)]
pub struct Ctx {
    pub app: AppHandle,
    shared: Arc<Mutex<Shared>>,
    pub audio: Audio,
    pub log: EventLog,
    pub status_item: MenuItem<Wry>,
}

impl Ctx {
    pub fn new(
        app: AppHandle,
        shared: Shared,
        audio: Audio,
        log: EventLog,
        status_item: MenuItem<Wry>,
    ) -> Self {
        Self {
            app,
            shared: Arc::new(Mutex::new(shared)),
            audio,
            log,
            status_item,
        }
    }

    /// A panic elsewhere must not wedge the alarm, so recover poisoned locks.
    pub fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
