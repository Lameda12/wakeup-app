// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod calibration;
mod commands;
mod monitor;
mod settings;
mod state;
mod store;
mod tray;
mod windows;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use crate::audio::Audio;
use crate::state::{Ctx, Shared};
use crate::store::EventLog;

fn main() {
    // SAFETY: first statement of main; no other threads exist yet.
    unsafe { wakeup_vision::disable_onnx_telemetry() };

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::alarm_info,
            commands::nudge_info,
            commands::typing_started,
            commands::submit_phrase,
            commands::get_settings,
            commands::save_settings,
            commands::calibration_start,
            commands::calibration_phase,
            commands::calibration_finish,
            commands::calibration_cancel,
            commands::finish_onboarding,
            commands::test_alarm,
            commands::open_calibration,
            commands::chime,
        ])
        .setup(|app| {
            // Menu bar app: no Dock icon, no app switcher entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let settings = store::load_settings(&config_dir);
            let log = EventLog::new(&app.path().app_data_dir()?);
            let status = tray::setup(&handle)?;
            windows::setup(&handle)?;

            if settings.onboarded {
                // Keep the login item in sync with the setting (e.g. after a reinstall).
                commands::set_autostart(&handle, settings.launch_at_login);
            } else {
                windows::open_onboarding(&handle, "onboarding");
            }

            let ctx = Ctx::new(
                handle,
                Shared::new(settings, config_dir),
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
                if window.label() == "onboarding" {
                    // Leaving setup midway must not leave the camera on.
                    if let Some(ctx) = window.app_handle().try_state::<Ctx>() {
                        let mut shared = ctx.lock();
                        if shared.calibration.take().is_some() {
                            shared.restart_engine();
                        }
                    }
                }
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
