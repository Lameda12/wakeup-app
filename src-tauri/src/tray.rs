use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use wakeup_core::{AlarmReason, PauseReason, State};

use crate::monitor::apply;
use crate::state::Ctx;
use crate::windows;

const ICON: &[u8] = include_bytes!("../icons/tray.png");

/// Builds the tray menu. Returns the status line so the monitor can update it.
pub fn setup(app: &AppHandle) -> tauri::Result<MenuItem<Wry>> {
    let status = MenuItem::with_id(app, "status", "starting…", false, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "pause-15", "Pause 15 minutes", true, None::<&str>)?,
            &MenuItem::with_id(app, "pause-60", "Pause 1 hour", true, None::<&str>)?,
            &MenuItem::with_id(app, "resume", "Resume", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "test", "Test alarm", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit wakeup", true, None::<&str>)?,
        ],
    )?;
    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip("wakeup")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu)
        .build(app)?;
    Ok(status)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let Some(ctx) = app.try_state::<Ctx>() else {
        return;
    };
    let ctx = ctx.inner().clone();
    let events = {
        let mut shared = ctx.lock();
        let now = shared.now();
        match event.id().as_ref() {
            "pause-15" => shared
                .engine
                .pause(PauseReason::Manual, Some(now + 15.0 * 60.0))
                .unwrap_or_default(),
            "pause-60" => shared
                .engine
                .pause(PauseReason::Manual, Some(now + 60.0 * 60.0))
                .unwrap_or_default(),
            "resume" => shared.engine.resume(),
            "test" => shared.engine.trigger(now, AlarmReason::EyesClosed),
            "settings" => {
                drop(shared);
                windows::open_settings(app);
                return;
            }
            "quit" => {
                drop(shared);
                app.exit(0);
                return;
            }
            _ => vec![],
        }
    };
    apply(&ctx, events);
}

pub fn status_label(state: &State, now: f64) -> String {
    match state {
        State::Active => "Active · camera off".into(),
        State::Watching => "Watching".into(),
        State::Nudge { .. } => "Still here?".into(),
        State::Alarm { .. } => "Alarm ringing".into(),
        State::Away => "Away · camera idle".into(),
        State::Paused {
            reason: PauseReason::Meeting,
            ..
        } => "Paused · on a call".into(),
        State::Paused {
            reason: PauseReason::Schedule,
            ..
        } => "Off · outside active hours".into(),
        State::Paused {
            until: Some(until), ..
        } => {
            format!(
                "Paused · {} min left",
                ((until - now) / 60.0).ceil().max(1.0) as u64
            )
        }
        State::Paused { until: None, .. } => "Paused".into(),
    }
}
