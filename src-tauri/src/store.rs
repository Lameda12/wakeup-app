//! Settings file and the local event log. Nothing leaves the machine.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use wakeup_core::{AlarmReason, Config};

use crate::state::wall_ms;

/// Loads `config.json`, writing the defaults first if it doesn't exist so
/// there's a file to edit. Missing keys fall back to defaults; a broken file
/// is reported and ignored rather than stopping the app.
pub fn load_config(dir: &Path) -> Config {
    let path = dir.join("config.json");
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!(
                "[config] {} is invalid ({e}); using defaults",
                path.display()
            );
            Config::default()
        }),
        Err(_) => {
            let cfg = Config::default();
            let written = fs::create_dir_all(dir).and_then(|()| {
                fs::write(
                    &path,
                    serde_json::to_string_pretty(&cfg).unwrap_or_default(),
                )
            });
            if let Err(e) = written {
                eprintln!("[config] couldn't write {}: {e}", path.display());
            }
            cfg
        }
    }
}

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
    fn config_round_trip_and_partial_files() {
        let dir = temp_dir("config");
        assert_eq!(load_config(&dir), Config::default());
        assert!(
            dir.join("config.json").exists(),
            "defaults written for editing"
        );

        fs::write(
            dir.join("config.json"),
            r#"{ "ear_closed": 0.17, "pitch_down": -20.0 }"#,
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.ear_closed, 0.17);
        assert_eq!(cfg.pitch_down, -20.0);
        assert_eq!(cfg.eyes_closed_for, Config::default().eyes_closed_for);

        fs::write(dir.join("config.json"), "{ not json").unwrap();
        assert_eq!(load_config(&dir), Config::default());
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
