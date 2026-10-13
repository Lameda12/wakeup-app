//! OS integration. M0 only needs idle time; volume, mic-in-use and overlay
//! helpers land here in later milestones.

/// Seconds since the last keyboard, mouse or trackpad event, or `None` where
/// this platform isn't supported yet.
pub fn idle_seconds() -> Option<f64> {
    imp::idle_seconds()
}

#[cfg(target_os = "macos")]
mod imp {
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT_EVENT: u32 = u32::MAX;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }

    pub fn idle_seconds() -> Option<f64> {
        // SAFETY: plain C function with value arguments, no preconditions.
        Some(unsafe {
            CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT)
        })
    }
}

// Linux (GNOME D-Bus, ext-idle-notify-v1, X11) is milestone M4.
#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn idle_seconds() -> Option<f64> {
        None
    }
}
