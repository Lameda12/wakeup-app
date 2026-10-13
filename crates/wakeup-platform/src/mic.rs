//! "Is some app recording from the microphone?" — the meeting signal.
//! Zoom, Meet, Teams, FaceTime and Discord calls all hold the mic open.

/// `None` when this system can't tell.
pub fn mic_in_use() -> Option<bool> {
    imp::mic_in_use()
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;

    #[repr(C)]
    struct PropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    const fn fourcc(code: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*code)
    }

    const SYSTEM_OBJECT: u32 = 1;
    const DEFAULT_INPUT_DEVICE: u32 = fourcc(b"dIn ");
    const DEVICE_IS_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const ELEMENT_MAIN: u32 = 0;

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    fn get_u32(object: u32, selector: u32) -> Option<u32> {
        let address = PropertyAddress {
            selector,
            scope: SCOPE_GLOBAL,
            element: ELEMENT_MAIN,
        };
        let mut value: u32 = 0;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: both properties are UInt32-sized (AudioObjectID / UInt32);
        // the out-pointer and size match.
        let status = unsafe {
            AudioObjectGetPropertyData(
                object,
                &address,
                0,
                ptr::null(),
                &mut size,
                (&mut value as *mut u32).cast(),
            )
        };
        (status == 0).then_some(value)
    }

    pub fn mic_in_use() -> Option<bool> {
        let device = get_u32(SYSTEM_OBJECT, DEFAULT_INPUT_DEVICE).filter(|&d| d != 0)?;
        get_u32(device, DEVICE_IS_RUNNING_SOMEWHERE).map(|running| running != 0)
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::process::Command;

    /// Any recording stream on PulseAudio (or PipeWire's Pulse layer).
    pub fn mic_in_use() -> Option<bool> {
        let out = Command::new("pactl")
            .args(["list", "short", "source-outputs"])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| !String::from_utf8_lossy(&out.stdout).trim().is_empty())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    pub fn mic_in_use() -> Option<bool> {
        None
    }
}
