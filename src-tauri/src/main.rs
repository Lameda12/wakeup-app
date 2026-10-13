// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod monitor;
mod state;
mod store;
mod tray;
mod windows;

use tauri::{Manager, State as TauriState};
use wakeup_core::{DismissError, Engine, matches_phrase};

use crate::audio::Audio;
use crate::state::{AlarmView, Ctx, NudgeView, Shared};
use crate::store::EventLog;

#[tauri::command]
fn alarm_info(ctx: TauriState<'_, Ctx>) -> Option<AlarmView> {
    ctx.lock().alarm.clone()
}

#[tauri::command]
fn nudge_info(ctx: TauriState<'_, Ctx>) -> Option<NudgeView> {
    ctx.lock().nudge.clone()
}

/// First keystroke of the phrase: from here on the camera checks your eyes.
#[tauri::command]
fn typing_started(ctx: TauriState<'_, Ctx>) {
    ctx.lock().engine.start_typing();
}

#[tauri::command]
fn submit_phrase(ctx: TauriState<'_, Ctx>, text: String) -> Result<(), String> {
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
            monitor::apply(&ctx, events);
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            alarm_info,
            nudge_info,
            typing_started,
            submit_phrase
        ])
        .setup(|app| {
            // Menu bar app: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let config = store::load_config(&app.path().app_config_dir()?);
            let log = EventLog::new(&app.path().app_data_dir()?);
            let status = tray::setup(&handle)?;
            windows::setup(&handle)?;

            let ctx = Ctx::new(
                handle,
                Shared::new(Engine::new(config)),
                Audio::spawn(),
                log,
                status,
            );
            app.manage(ctx.clone());
            monitor::spawn(ctx);
            Ok(())
        })
        // Cmd+W must neither quit the app nor make the alarm go away; only
        // the phrase does that. (Force Quit still works: it's your machine.)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if !window.label().starts_with("alarm-") {
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start wakeup")
        .run(|_, event| {
            // Keep running with no windows visible: it lives in the tray.
            if let tauri::RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
