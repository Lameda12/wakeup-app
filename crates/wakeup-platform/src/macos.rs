//! AppKit bits Tauri doesn't expose.

use std::ffi::c_void;

use objc2::msg_send;
use objc2::runtime::AnyObject;

/// `NSScreenSaverWindowLevel`: above the menu bar, the Dock and full-screen apps.
const SCREEN_SAVER_LEVEL: isize = 1000;
/// `NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary`.
const ALL_SPACES_AND_FULLSCREEN: usize = (1 << 0) | (1 << 8);

/// Lifts a window above everything, on every Space, including over
/// full-screen apps.
///
/// # Safety
/// `ns_window` must be a live `NSWindow` pointer (e.g. from Tauri's
/// `WebviewWindow::ns_window`), and this must run on the main thread.
pub unsafe fn float_above_everything(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    // SAFETY: caller guarantees a live NSWindow on the main thread; both
    // selectors exist on NSWindow and take plain integer arguments.
    unsafe {
        let window = &*(ns_window as *const AnyObject);
        let _: () = msg_send![window, setLevel: SCREEN_SAVER_LEVEL];
        let _: () = msg_send![window, setCollectionBehavior: ALL_SPACES_AND_FULLSCREEN];
    }
}
