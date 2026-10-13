//! The alarm (one full-screen window per display) and the nudge card.

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Monitor, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

const NUDGE: &str = "nudge";
const NUDGE_SIZE: (f64, f64) = (360.0, 120.0);
const NUDGE_MARGIN: f64 = 24.0;

fn build(app: &AppHandle, label: &str, view: &str) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html#{view}").into()),
    )
    .title("wakeup")
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible_on_all_workspaces(true)
    .shadow(false)
    .focused(false)
    // The nudge must never take the keyboard: answering it means touching a
    // key in whatever you were doing.
    .focusable(label != NUDGE)
    .visible(false)
    .build()
}

/// Pre-creates the hidden windows so the alarm appears instantly.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    build(app, "alarm-0", "alarm")?;
    build(app, NUDGE, "nudge")?;
    Ok(())
}

fn same_monitor(a: &Monitor, b: &Monitor) -> bool {
    a.position() == b.position() && a.size() == b.size()
}

/// Covers every display. The primary display's window takes the keyboard
/// for the dismiss phrase; the others just flash.
pub fn show_alarm(app: &AppHandle) {
    let monitors = app.available_monitors().unwrap_or_default();
    let primary = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.first().cloned());
    // Primary first, so it gets label alarm-0 (pre-created with the input).
    let mut ordered: Vec<Monitor> = primary.iter().cloned().collect();
    ordered.extend(
        monitors
            .into_iter()
            .filter(|m| primary.as_ref().is_none_or(|p| !same_monitor(m, p))),
    );

    for (i, monitor) in ordered.iter().enumerate() {
        let view = if i == 0 { "alarm" } else { "alarm-secondary" };
        let window = match build(app, &format!("alarm-{i}"), view) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[windows] alarm window {i}: {e}");
                continue;
            }
        };
        let _ = window.set_position(*monitor.position());
        let _ = window.set_size(*monitor.size());
        let _ = window.show();
        raise(app, &window);
        if i == 0 {
            let _ = window.set_focus();
        }
    }
}

pub fn hide_alarm(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("alarm-") {
            let _ = window.hide();
        }
    }
}

/// Top-right corner of the primary display, without taking focus.
pub fn show_nudge(app: &AppHandle) {
    let Some(window) = app.get_webview_window(NUDGE) else {
        return;
    };
    if let Some(m) = app.primary_monitor().ok().flatten() {
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        let _ = window.set_size(LogicalSize::new(NUDGE_SIZE.0, NUDGE_SIZE.1));
        let _ = window.set_position(LogicalPosition::new(
            pos.x + size.width - NUDGE_SIZE.0 - NUDGE_MARGIN,
            pos.y + NUDGE_MARGIN + 28.0, // clear the macOS menu bar
        ));
    }
    let _ = window.show();
    raise(app, &window);
}

pub fn hide_nudge(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(NUDGE) {
        let _ = window.hide();
    }
}

/// macOS: above full-screen apps and on every Space. Elsewhere, Tauri's
/// always-on-top is what's available (see DESIGN.md for Linux caveats).
fn raise(app: &AppHandle, window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        let window = window.clone();
        let _ = app.run_on_main_thread(move || {
            if let Ok(ns) = window.ns_window() {
                // SAFETY: Tauri hands out the live NSWindow; we're on the main thread.
                unsafe { wakeup_platform::macos::float_above_everything(ns) };
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, window);
}

/// A normal app window (settings, onboarding): created on demand, reused,
/// hidden rather than closed.
fn open_panel(app: &AppHandle, label: &str, view: &str, size: (f64, f64)) {
    let window = match app.get_webview_window(label) {
        Some(w) => w,
        None => {
            let built = WebviewWindowBuilder::new(
                app,
                label,
                WebviewUrl::App(format!("index.html#{view}").into()),
            )
            .title("wakeup")
            .inner_size(size.0, size.1)
            .resizable(false)
            .center()
            .build();
            match built {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[windows] {label}: {e}");
                    return;
                }
            }
        }
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

pub fn open_settings(app: &AppHandle) {
    open_panel(app, "settings", "settings", (560.0, 760.0));
}

pub fn open_onboarding(app: &AppHandle, view: &str) {
    // Re-point an existing window if switching between full setup and recalibration.
    if let Some(w) = app.get_webview_window("onboarding") {
        let _ = w.eval(format!("location.hash = '{view}'; location.reload();"));
    }
    open_panel(app, "onboarding", view, (640.0, 620.0));
}

pub fn close(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.hide();
    }
}
