//! Settings file and the local event log. Nothing leaves the machine.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use wakeup_core::AlarmReason;

use crate::settings::Settings;
use crate::state::wall_ms;

/// Loads `config.json`, writing the defaults first if it doesn't exist so
/// there's a file to edit. Missing keys fall back to defaults; a broken file
/// is reported and ignored rather than stopping the app.
pub fn load_settings(dir: &Path) -> Settings {
    let path = dir.join(FILE);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!(
                "[config] {} is invalid ({e}); using defaults",
                path.display()
            );
            Settings::default()
        }),
        Err(_) => {
            let settings = Settings::default();
            if let Err(e) = save_settings(dir, &settings) {
                eprintln!("[config] couldn't write {}: {e}", path.display());
            }
            settings
        }
    }
}

/// Atomic write: a crash mid-save can't leave a half-written file.
pub fn save_settings(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    fs::write(&tmp, serde_json::to_string_pretty(settings)?)?;
    fs::rename(tmp, dir.join(FILE))
}

const FILE: &str = "config.json";

/// Append-only JSON lines: one per alarm, for "you dozed 3x today" and
/// for tuning thresholds.
#[derive(Clone)]
pub struct EventLog(Arc<Mutex<PathBuf>>);

impl EventLog {
    pub fn new(dir: &Path) -> Self {
        let _ = fs::create_dir_all(dir);
        Self(Arc::new(Mutex::new(dir.join("events.jsonl"))))
    }

    pub fn alarm(&self, reason: AlarmReason) {
        self.write(json!({ "ts": wall_ms(), "event": "alarm", "reason": reason }));
    }

    pub fn dismissed(&self, reason: AlarmReason, after: f64) {
        self.write(json!({ "ts": wall_ms(), "event": "dismissed", "reason": reason, "secs": (after * 10.0).round() / 10.0 }));
    }

    pub fn failed_dismiss(&self, open_fraction: f32) {
        self.write(json!({ "ts": wall_ms(), "event": "dismiss_rejected", "eyes_open": (open_fraction * 100.0).round() / 100.0 }));
    }

    fn write(&self, line: serde_json::Value) {
        let path = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&*path)
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = result {
            eprintln!("[log] {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wakeup-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn settings_round_trip_and_partial_files() {
        let dir = temp_dir("config");
        assert_eq!(load_settings(&dir), Settings::default());
        assert!(
            dir.join("config.json").exists(),
            "defaults written for editing"
        );

        fs::write(
            dir.join("config.json"),
            r#"{ "ear_closed": 0.17, "onboarded": true }"#,
        )
        .unwrap();
        let s = load_settings(&dir);
        assert_eq!(s.engine.ear_closed, 0.17);
        assert!(s.onboarded);
        assert_eq!(
            s.engine.eyes_closed_for,
            Settings::default().engine.eyes_closed_for
        );

        let changed = Settings {
            meeting_pause: false,
            ..s
        };
        save_settings(&dir, &changed).unwrap();
        assert_eq!(load_settings(&dir), changed);
        assert!(!dir.join("config.json.tmp").exists());

        fs::write(dir.join("config.json"), "{ not json").unwrap();
        assert_eq!(load_settings(&dir), Settings::default());
    }

    #[test]
    fn log_appends_lines() {
        let dir = temp_dir("log");
        let log = EventLog::new(&dir);
        log.alarm(AlarmReason::HeadDown);
        log.dismissed(AlarmReason::HeadDown, 12.34);
        let text = fs::read_to_string(dir.join("events.jsonl")).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["reason"], "HeadDown");
        assert_eq!(lines[1]["secs"], 12.3);
    }
}
