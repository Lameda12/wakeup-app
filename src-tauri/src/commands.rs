//! Everything the UI can ask the app to do.

use tauri::{AppHandle, State as TauriState};
use wakeup_core::{AlarmReason, CalibrationResult, DismissError, calibrate, matches_phrase};

use crate::calibration::{Phase, Session};
use crate::monitor::{self, apply};
use crate::settings::Settings;
use crate::state::{AlarmView, Ctx, NudgeView};
use crate::windows;

type Ctxs<'a> = TauriState<'a, Ctx>;

#[tauri::command]
pub fn alarm_info(ctx: Ctxs<'_>) -> Option<AlarmView> {
    ctx.lock().alarm.clone()
}

#[tauri::command]
pub fn nudge_info(ctx: Ctxs<'_>) -> Option<NudgeView> {
    ctx.lock().nudge.clone()
}

/// First keystroke of the phrase: from here on the camera checks your eyes.
#[tauri::command]
pub fn typing_started(ctx: Ctxs<'_>) {
    ctx.lock().engine.start_typing();
}

#[tauri::command]
pub fn submit_phrase(ctx: Ctxs<'_>, text: String) -> Result<(), String> {
    let ctx = ctx.inner().clone();
    let result = {
        let mut shared = ctx.lock();
        let phrase = shared
            .alarm
            .as_ref()
            .map(|a| a.phrase.clone())
            .ok_or("no alarm is ringing")?;
        if !matches_phrase(&text, &phrase) {
            return Err("that's not the phrase. try again.".into());
        }
        let now = shared.now();
        shared.engine.try_dismiss(now)
    };
    match result {
        Ok(events) => {
            apply(&ctx, events);
            Ok(())
        }
        Err(DismissError::EyesNotOpen { open_fraction }) => {
            ctx.log.failed_dismiss(open_fraction);
            Err(format!(
                "your eyes were open only {:.0}% of the time. type it again, eyes open.",
                open_fraction * 100.0
            ))
        }
        Err(DismissError::NoAlarm) => Err("no alarm is ringing".into()),
    }
}

#[tauri::command]
pub fn get_settings(ctx: Ctxs<'_>) -> Settings {
    ctx.lock().settings.clone()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    ctx: Ctxs<'_>,
    settings: Settings,
) -> Result<Settings, String> {
    let login_changed = {
        let mut shared = ctx.lock();
        let changed = shared.settings.launch_at_login != settings.launch_at_login;
        shared.apply_settings(settings)?;
        changed
    };
    if login_changed {
        set_autostart(&app, ctx.lock().settings.launch_at_login);
    }
    // Toggling calls/active hours takes effect now, not at the next poll.
    monitor::check_meeting(&ctx);
    monitor::check_schedule(&ctx);
    Ok(ctx.lock().settings.clone())
}

#[tauri::command]
pub fn calibration_start(app: AppHandle, ctx: Ctxs<'_>) -> Result<(), String> {
    let mut shared = ctx.lock();
    if shared.alarm.is_some() {
        return Err("dismiss the alarm first".into());
    }
    shared.calibration = Some(Session::default());
    shared.nudge = None;
    drop(shared);
    windows::hide_nudge(&app);
    Ok(())
}

#[tauri::command]
pub fn calibration_phase(ctx: Ctxs<'_>, phase: Phase) -> Result<(), String> {
    let mut shared = ctx.lock();
    let session = shared
        .calibration
        .as_mut()
        .ok_or("calibration isn't running")?;
    session.set_phase(phase);
    eprintln!("[calibration] phase {phase:?}");
    Ok(())
}

/// Computes thresholds from the recording and saves them. Either way the
/// engine restarts fresh, since the camera was busy calibrating.
#[tauri::command]
pub fn calibration_finish(ctx: Ctxs<'_>) -> Result<CalibrationResult, String> {
    let mut shared = ctx.lock();
    let session = shared
        .calibration
        .take()
        .ok_or("calibration isn't running")?;
    shared.restart_engine();
    let (open, closed, down) = (
        session.data.open.len(),
        session.data.closed.len(),
        session.data.down.len(),
    );
    eprintln!("[calibration] samples open {open}, closed {closed}, down {down}");
    let result = calibrate(&session.data).map_err(|e| e.to_string())?;
    eprintln!("[calibration] ok: {result:?}");

    let mut settings = shared.settings.clone();
    settings.engine.ear_closed = result.ear_closed;
    settings.engine.pitch_neutral = result.pitch_neutral;
    // Down == neutral switches the head-down rule off (see Config::is_head_down).
    settings.engine.pitch_down = result.pitch_down.unwrap_or(result.pitch_neutral);
    settings.calibrated = true;
    shared.apply_settings(settings)?;
    shared.restart_engine();
    Ok(result)
}

#[tauri::command]
pub fn calibration_cancel(ctx: Ctxs<'_>) {
    let mut shared = ctx.lock();
    if shared.calibration.take().is_some() {
        shared.restart_engine();
    }
}

#[tauri::command]
pub fn finish_onboarding(
    app: AppHandle,
    ctx: Ctxs<'_>,
    launch_at_login: bool,
) -> Result<(), String> {
    {
        let mut shared = ctx.lock();
        let settings = Settings {
            onboarded: true,
            launch_at_login,
            ..shared.settings.clone()
        };
        shared.apply_settings(settings)?;
    }
    set_autostart(&app, launch_at_login);
    windows::close(&app, "onboarding");
    Ok(())
}

#[tauri::command]
pub fn test_alarm(ctx: Ctxs<'_>) -> Result<(), String> {
    let ctx = ctx.inner().clone();
    let events = {
        let mut shared = ctx.lock();
        if shared.calibration.is_some() {
            return Err("finish calibrating first".into());
        }
        let now = shared.now();
        shared.engine.trigger(now, AlarmReason::EyesClosed)
    };
    apply(&ctx, events);
    Ok(())
}

#[tauri::command]
pub fn open_calibration(app: AppHandle) {
    windows::open_onboarding(&app, "calibrate");
}

/// Audible cue between calibration phases (eyes are closed for one of them).
#[tauri::command]
pub fn chime(ctx: Ctxs<'_>) {
    ctx.audio.chime();
}

/// Registers or removes the login item. Skipped in debug builds so a dev
/// binary never ends up launching at login.
pub fn set_autostart(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(e) = result {
        eprintln!("[autostart] {e}");
    }
}
